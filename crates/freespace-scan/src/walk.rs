//! Parallel directory-walk scanner used for non-NTFS volumes, unprivileged
//! runs and sub-directory scans.
//!
//! `jwalk` reads directories in parallel while the consumer builds the arena
//! sequentially, which keeps the hot path allocation-free apart from the
//! directory index (FR-004, FR-010).

use std::{
    collections::{HashMap, HashSet},
    fs,
    os::windows::fs::MetadataExt,
    path::{Path, PathBuf},
    time::Duration,
};

use freespace_core::{
    Error, Result, Timestamp,
    model::{ScanIssue, ScanIssueKind},
    size::SizeMetrics,
    tree::{AccessState, FileAttributes, FsTree, NodeId, NodeKind, NodeSpec, Timestamps},
};
use freespace_platform::{paths, win};
use globset::{GlobBuilder, GlobMatcher};
use jwalk::{Parallelism, WalkDir};

use crate::session::{ScanObserver, ScanPhase, ScanProgress};

/// Update progress at least every this many entries.
const PROGRESS_INTERVAL: u64 = 4_096;

/// Compiled exclusion patterns.
#[derive(Debug, Clone, Default)]
pub struct Exclusions {
    matchers: Vec<GlobMatcher>,
    patterns: Vec<String>,
}

impl Exclusions {
    /// Compiles `--exclude` globs.
    ///
    /// # Errors
    /// Returns [`Error::InvalidArgument`] for a malformed glob.
    pub fn new(patterns: &[String]) -> Result<Self> {
        let mut matchers = Vec::with_capacity(patterns.len());
        let mut normalized = Vec::with_capacity(patterns.len());

        for pattern in patterns {
            if pattern.trim().is_empty() {
                continue;
            }
            let pattern = pattern.replace('\\', "/");
            let matcher = GlobBuilder::new(&pattern)
                .case_insensitive(true)
                .literal_separator(false)
                .backslash_escape(false)
                .build()
                .map_err(|error| {
                    Error::InvalidArgument(format!("invalid exclusion `{pattern}`: {error}"))
                })?
                .compile_matcher();
            matchers.push(matcher);
            normalized.push(pattern);
        }

        Ok(Self {
            matchers,
            patterns: normalized,
        })
    }

    /// Returns `true` when no exclusion was configured.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.matchers.is_empty()
    }

    /// Original patterns, useful for reporting.
    #[must_use]
    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }

    /// Returns `true` when the name or the full path matches an exclusion.
    #[must_use]
    pub fn is_excluded(&self, path: &Path, name: &str) -> bool {
        if self.matchers.is_empty() {
            return false;
        }
        if self.matchers.iter().any(|matcher| matcher.is_match(name)) {
            return true;
        }

        let normalized = path.to_string_lossy().replace('\\', "/");

        self.matchers
            .iter()
            .any(|matcher| matcher.is_match(&normalized))
    }
}

/// Result of a walk.
#[derive(Debug, Default)]
pub struct WalkOutcome {
    /// Number of excluded entries (including their subtrees).
    pub excluded: u64,
    /// Non-fatal read problems.
    pub issues: Vec<ScanIssue>,
    /// Whether the walk stopped because cancellation was requested.
    pub cancelled: bool,
}

/// Walks `root` and appends nodes to `tree`.
///
/// # Errors
/// Returns an error when the root cannot be read or the root node cannot be
/// added.
pub fn scan(
    root: &Path,
    tree: &mut FsTree,
    exclusions: &Exclusions,
    observer: &mut dyn ScanObserver,
) -> Result<WalkOutcome> {
    let root = paths::normalize(root)?;
    let metadata = fs::metadata(&root)?;
    if !metadata.is_dir() {
        return Err(Error::InvalidArgument(format!(
            "`{}` is not a directory",
            paths::display(&root)
        )));
    }

    let mut outcome = WalkOutcome::default();
    let root_id = tree.add_node(None, node_spec(&root, &metadata, None))?;
    let mut directories: HashMap<PathBuf, NodeId> = HashMap::new();
    directories.insert(root.clone(), root_id);
    let mut excluded_directories: HashSet<PathBuf> = HashSet::new();
    let mut link_directories: HashSet<PathBuf> = HashSet::new();
    let mut nodes = 1u64;
    let mut bytes = 0u64;
    let mut last_report = 0u64;

    let walker = WalkDir::new(&root)
        .skip_hidden(false)
        .follow_links(false)
        .sort(false)
        .parallelism(Parallelism::RayonDefaultPool {
            busy_timeout: Duration::from_millis(100),
        });

    for entry in walker {
        if observer.is_cancelled() {
            outcome.cancelled = true;
            break;
        }

        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                outcome.issues.push(walk_issue(&error));
                continue;
            }
        };

        if entry.depth() == 0 {
            continue;
        }

        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();

        if excluded_directories
            .iter()
            .any(|excluded| path.starts_with(excluded))
        {
            outcome.excluded = outcome.excluded.saturating_add(1);
            continue;
        }
        if link_directories.iter().any(|link| path.starts_with(link)) {
            // Reparse points are never traversed: following them could
            // duplicate gigabytes or create cycles.
            continue;
        }
        if exclusions.is_excluded(&path, &name) {
            if entry.file_type().is_dir() {
                excluded_directories.insert(path.clone());
            }
            outcome.excluded = outcome.excluded.saturating_add(1);
            continue;
        }

        let Some(parent_path) = path.parent() else {
            outcome.issues.push(ScanIssue {
                path: path.clone(),
                kind: ScanIssueKind::Other,
                message: "entry has no parent directory".to_owned(),
            });
            continue;
        };
        let Some(parent_id) = directories.get(parent_path).copied() else {
            outcome.issues.push(ScanIssue {
                path: path.clone(),
                kind: ScanIssueKind::Other,
                message: "parent directory was not scanned".to_owned(),
            });
            continue;
        };

        let entry_metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                outcome.issues.push(ScanIssue {
                    path: path.clone(),
                    kind: ScanIssueKind::AccessDenied,
                    message: format!("{} ({error})", crate::access::denial_message(&path)),
                });
                let spec = NodeSpec {
                    name: name.clone(),
                    kind: if entry.file_type().is_dir() {
                        NodeKind::Directory
                    } else {
                        NodeKind::File
                    },
                    access: AccessState::Denied,
                    ..NodeSpec::default()
                };
                tree.add_node(Some(parent_id), spec)?;
                nodes += 1;
                continue;
            }
        };

        let spec = node_spec(&path, &entry_metadata, path.file_name());
        let is_directory = spec.kind.is_directory();
        let is_reparse = spec.attributes.contains(FileAttributes::REPARSE_POINT);
        let size = spec.size.logical;
        let id = tree.add_node(Some(parent_id), spec)?;
        nodes += 1;
        bytes = bytes.saturating_add(size);

        if is_directory {
            directories.insert(path.clone(), id);
        } else if is_reparse && entry_metadata.is_dir() {
            link_directories.insert(path.clone());
        }

        if nodes.saturating_sub(last_report) >= PROGRESS_INTERVAL {
            last_report = nodes;
            observer.progress(&ScanProgress {
                phase: ScanPhase::Building,
                nodes,
                bytes,
                fraction: 0.5,
                current: path.clone(),
            });
        }
    }

    Ok(outcome)
}

fn node_spec(path: &Path, metadata: &fs::Metadata, name: Option<&std::ffi::OsStr>) -> NodeSpec {
    let attributes = FileAttributes::from_windows_attributes(metadata.file_attributes());
    let is_reparse = attributes.contains(FileAttributes::REPARSE_POINT);
    let kind = if metadata.is_dir() {
        if is_reparse {
            NodeKind::Junction
        } else {
            NodeKind::Directory
        }
    } else if is_reparse || metadata.file_type().is_symlink() {
        NodeKind::Symlink
    } else {
        NodeKind::File
    };

    let size = if kind.is_directory() || kind == NodeKind::Junction {
        SizeMetrics::ZERO
    } else {
        let bytes = metadata.len();
        SizeMetrics::exact(bytes, bytes, bytes, 0)
    };

    let name = name
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| paths::display(path));

    NodeSpec {
        name,
        kind,
        size,
        timestamps: Timestamps {
            created: Timestamp::from_millis(win::filetime_to_millis(metadata.creation_time())),
            modified: Timestamp::from_millis(win::filetime_to_millis(metadata.last_write_time())),
            accessed: Timestamp::from_millis(win::filetime_to_millis(metadata.last_access_time())),
        },
        attributes,
        access: AccessState::Ok,
        ..NodeSpec::default()
    }
}

fn walk_issue(error: &jwalk::Error) -> ScanIssue {
    let path = error
        .path()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let kind = match error.io_error() {
        Some(io) if io.kind() == std::io::ErrorKind::PermissionDenied => {
            ScanIssueKind::AccessDenied
        }
        Some(_) => ScanIssueKind::Io,
        None => ScanIssueKind::Other,
    };

    ScanIssue {
        path,
        kind,
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use freespace_core::tree::FsTree;

    use super::{Exclusions, scan};
    use crate::session::{NoopObserver, ScanObserver};

    fn fixture() -> TempDir {
        let directory = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(directory.path().join("alpha")).expect("alpha");
        std::fs::create_dir_all(directory.path().join("beta").join("nested")).expect("beta");
        std::fs::create_dir_all(directory.path().join("empty")).expect("empty");
        std::fs::write(directory.path().join("alpha").join("first.txt"), [b'a'; 10])
            .expect("first");
        std::fs::write(
            directory
                .path()
                .join("beta")
                .join("nested")
                .join("deep.bin"),
            [b'b'; 20],
        )
        .expect("deep");
        std::fs::write(directory.path().join("root.txt"), [b'c'; 5]).expect("root file");

        directory
    }

    #[test]
    fn builds_a_tree_with_aggregated_sizes() {
        let directory = fixture();
        let mut tree = FsTree::new();
        let exclusions = Exclusions::default();

        let outcome =
            scan(directory.path(), &mut tree, &exclusions, &mut NoopObserver).expect("scan");
        tree.recompute_totals();

        let root = tree.roots().next().expect("root");
        assert_eq!(tree.node(root).size.logical, 35);
        assert_eq!(tree.node(root).file_count(), 3);
        assert_eq!(tree.node(root).dir_count(), 4);
        assert!(outcome.issues.is_empty());
        assert_eq!(tree.live_len(), 8);
        tree.validate().expect("consistent tree");
    }

    #[test]
    fn applies_name_and_path_exclusions() {
        let directory = fixture();
        let mut tree = FsTree::new();
        let exclusions = Exclusions::new(&["beta".to_owned(), "*.txt".to_owned()]).expect("glob");

        let outcome =
            scan(directory.path(), &mut tree, &exclusions, &mut NoopObserver).expect("scan");
        tree.recompute_totals();

        let root = tree.roots().next().expect("root");
        assert_eq!(tree.node(root).size.logical, 0);
        assert_eq!(tree.node(root).file_count(), 0);
        assert!(outcome.excluded >= 3);
        assert_eq!(exclusions.patterns().len(), 2);
        assert!(!exclusions.is_empty());
    }

    #[test]
    fn marks_inaccessible_directories() {
        let directory = fixture();
        let missing = directory.path().join("missing");
        let mut tree = FsTree::new();

        assert!(
            scan(
                &missing,
                &mut tree,
                &Exclusions::default(),
                &mut NoopObserver
            )
            .is_err()
        );
    }

    #[test]
    fn stops_on_cancellation() {
        struct Cancelling(bool);

        impl ScanObserver for Cancelling {
            fn is_cancelled(&self) -> bool {
                self.0
            }
        }

        let directory = fixture();
        let mut tree = FsTree::new();
        let mut observer = Cancelling(false);
        let result = scan(
            directory.path(),
            &mut tree,
            &Exclusions::default(),
            &mut observer,
        )
        .expect("partial scan");

        assert!(!result.cancelled);
        observer.0 = true;
        let mut second = FsTree::new();
        let cancelled = scan(
            directory.path(),
            &mut second,
            &Exclusions::default(),
            &mut observer,
        )
        .expect("cancelled scan");
        assert!(cancelled.cancelled);
        assert_eq!(second.live_len(), 1, "only the root is added");
    }

    #[test]
    fn rejects_invalid_globs() {
        assert!(Exclusions::new(&["[".to_owned()]).is_err());
        assert!(
            Exclusions::new(&["  ".to_owned()])
                .expect("blank patterns are ignored")
                .is_empty()
        );
    }
}
