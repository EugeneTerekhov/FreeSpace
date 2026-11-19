//! Incremental updates from the NTFS USN change journal.
//!
//! A full MFT pass is fast, but a repeat scan only needs the records written
//! since the previous run. [`apply`] is a pure function over a list of change
//! records, so it can be tested without a volume; [`update`] reads the journal
//! and then calls it (FR-009).

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use freespace_core::{
    Result, Timestamp,
    model::{JournalState, ScanIssue, ScanIssueKind},
    size::SizeMetrics,
    tree::{AccessState, FileAttributes, FsTree, NodeId, NodeKind, NodeSpec},
};
use freespace_platform::mft::{
    self, USN_REASON_DATA_EXTEND, USN_REASON_DATA_OVERWRITE, USN_REASON_DATA_TRUNCATION,
    USN_REASON_FILE_CREATE, USN_REASON_FILE_DELETE, USN_REASON_RENAME_NEW_NAME, UsnRecord,
};

use freespace_platform::paths;

/// Outcome of applying a journal delta.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct UsnUpdateReport {
    /// Objects added to the tree.
    pub added: u64,
    /// Objects removed from the tree.
    pub removed: u64,
    /// Objects whose metadata changed.
    pub changed: u64,
    /// Whether the delta could not be applied completely.
    pub requires_rescan: bool,
    /// Journal position to store for the next update.
    pub state: Option<JournalState>,
    /// Records that could not be applied.
    pub issues: Vec<ScanIssue>,
}

/// Captures the current journal position of the volume that contains `root`.
///
/// # Errors
/// Returns an error when the volume cannot be queried.
pub fn capture_state(root: &Path) -> Result<JournalState> {
    mft::query_journal(root)
}

/// Reads the journal delta since `state` and applies it to `tree`.
///
/// # Errors
/// Returns an error when the journal cannot be read.
pub fn update(root: &Path, tree: &mut FsTree, state: JournalState) -> Result<UsnUpdateReport> {
    let journal = mft::read_journal(root, state)?;
    let mut report = apply(tree, &journal.records);
    report.requires_rescan |= journal.requires_rescan;
    report.state = Some(journal.state);

    Ok(report)
}

/// Applies change records to a tree.
///
/// Unknown parents cannot be resolved without a full scan, so the report is
/// marked with `requires_rescan` and the affected object is skipped.
pub fn apply(tree: &mut FsTree, changes: &[UsnRecord]) -> UsnUpdateReport {
    let mut report = UsnUpdateReport::default();
    let mut by_file_ref = index_by_file_ref(tree);
    let mut directories = index_directories(tree);

    for change in changes {
        if change.reason & USN_REASON_FILE_DELETE != 0 {
            if let Some(nodes) = by_file_ref.remove(&change.file_ref) {
                for node in nodes {
                    report.removed = report
                        .removed
                        .saturating_add(u64::try_from(tree.mark_removed(node)).unwrap_or(0));
                }
                directories.remove(&change.file_ref);
            }
            continue;
        }

        let created = change.reason & USN_REASON_FILE_CREATE != 0;
        let renamed = change.reason & USN_REASON_RENAME_NEW_NAME != 0;
        let data_changed = change.reason
            & (USN_REASON_DATA_EXTEND | USN_REASON_DATA_TRUNCATION | USN_REASON_DATA_OVERWRITE)
            != 0;

        let existing = by_file_ref
            .get(&change.file_ref)
            .cloned()
            .unwrap_or_default();
        if existing.is_empty() {
            if !created && !renamed {
                continue;
            }
            let parent = directories.get(&change.parent_ref).copied();
            match create_node(tree, change, parent) {
                Ok(node) => {
                    by_file_ref.entry(change.file_ref).or_default().push(node);
                    if tree.node(node).kind.is_directory() {
                        directories.entry(change.file_ref).or_insert(node);
                    }
                    report.added = report.added.saturating_add(1);
                }
                Err(issue) => {
                    report.requires_rescan = true;
                    report.issues.push(issue);
                }
            }
            continue;
        }

        for node in existing {
            let parent = directories.get(&change.parent_ref).copied();
            if let Some(parent) = parent {
                if tree.node(node).parent() != Some(parent) {
                    if let Err(error) = tree.set_parent(node, parent) {
                        report.issues.push(ScanIssue {
                            path: tree.path(node),
                            kind: ScanIssueKind::Other,
                            message: error.to_string(),
                        });
                    }
                }
            }
            if renamed || tree.name(node) != change.name {
                if let Err(error) = tree.set_name(node, &change.name) {
                    report.issues.push(ScanIssue {
                        path: tree.path(node),
                        kind: ScanIssueKind::Other,
                        message: error.to_string(),
                    });
                }
            }
            if data_changed || created || renamed {
                refresh_metadata(tree, node);
                report.changed = report.changed.saturating_add(1);
            }
        }
    }

    tree.recompute_totals();

    report
}

fn index_directories(tree: &FsTree) -> HashMap<u64, NodeId> {
    let mut index = HashMap::new();
    for id in 0..tree.len() as u32 {
        let node = tree.node(id);
        if node.is_removed() || !node.kind.is_directory() || node.file_ref == 0 {
            continue;
        }
        index.entry(node.file_ref).or_insert(id);
    }

    index
}

fn index_by_file_ref(tree: &FsTree) -> HashMap<u64, Vec<NodeId>> {
    let mut index: HashMap<u64, Vec<NodeId>> = HashMap::new();
    for id in 0..tree.len() as u32 {
        let node = tree.node(id);
        if node.is_removed() || node.file_ref == 0 {
            continue;
        }
        index.entry(node.file_ref).or_default().push(id);
    }

    index
}

fn create_node(
    tree: &mut FsTree,
    change: &UsnRecord,
    parent: Option<NodeId>,
) -> std::result::Result<NodeId, ScanIssue> {
    let parent = parent.ok_or_else(|| ScanIssue {
        path: PathBuf::from(&change.name),
        kind: ScanIssueKind::Other,
        message: format!(
            "parent record {} of `{}` is unknown; a full scan is required",
            change.parent_ref, change.name
        ),
    })?;

    let path = tree.path(parent).join(&change.name);
    let (size, timestamps, attributes, access) = match fs::symlink_metadata(&path) {
        Ok(metadata) => (
            SizeMetrics::file(metadata.len()),
            freespace_core::tree::Timestamps {
                created: Timestamp::from_millis(freespace_platform::win::filetime_to_millis(
                    std::os::windows::fs::MetadataExt::creation_time(&metadata),
                )),
                modified: Timestamp::from_millis(freespace_platform::win::filetime_to_millis(
                    std::os::windows::fs::MetadataExt::last_write_time(&metadata),
                )),
                accessed: Timestamp::from_millis(freespace_platform::win::filetime_to_millis(
                    std::os::windows::fs::MetadataExt::last_access_time(&metadata),
                )),
            },
            FileAttributes::from_windows_attributes(
                std::os::windows::fs::MetadataExt::file_attributes(&metadata),
            ),
            AccessState::Ok,
        ),
        Err(_) => (
            SizeMetrics::ZERO,
            freespace_core::tree::Timestamps::default(),
            change.attributes,
            AccessState::Denied,
        ),
    };

    let kind = if attributes.contains(FileAttributes::DIRECTORY) {
        NodeKind::Directory
    } else {
        NodeKind::File
    };
    let spec = NodeSpec {
        name: change.name.clone(),
        kind,
        file_ref: change.file_ref,
        size: if kind.is_directory() {
            SizeMetrics::ZERO
        } else {
            size
        },
        timestamps,
        attributes,
        access,
        ..NodeSpec::default()
    };

    let node = tree
        .add_node(Some(parent), spec)
        .map_err(|error| ScanIssue {
            path: paths::display(&path).into(),
            kind: ScanIssueKind::Other,
            message: error.to_string(),
        })?;

    Ok(node)
}

fn refresh_metadata(tree: &mut FsTree, node: NodeId) {
    let path = tree.path(node);
    let Ok(metadata) = fs::symlink_metadata(&path) else {
        return;
    };
    let attributes = FileAttributes::from_windows_attributes(
        std::os::windows::fs::MetadataExt::file_attributes(&metadata),
    );
    let record = tree.node_mut(node);
    if record.kind.is_directory() {
        return;
    }
    let bytes = metadata.len();
    record.size = SizeMetrics::file(bytes);
    record.attributes = attributes;
    record.access = AccessState::Ok;
    record.timestamps.modified =
        Timestamp::from_millis(freespace_platform::win::filetime_to_millis(
            std::os::windows::fs::MetadataExt::last_write_time(&metadata),
        ));
}

#[cfg(test)]
mod tests {
    use std::fs;

    use freespace_core::{
        Timestamp,
        size::SizeMetrics,
        tree::{FsTree, NodeKind, NodeSpec},
    };
    use freespace_platform::mft::{
        USN_REASON_DATA_EXTEND, USN_REASON_FILE_CREATE, USN_REASON_FILE_DELETE,
        USN_REASON_RENAME_NEW_NAME, UsnRecord,
    };
    use freespace_platform::paths;

    use super::apply;

    fn record(file_ref: u64, parent_ref: u64, name: &str, reason: u32) -> UsnRecord {
        UsnRecord {
            file_ref,
            parent_ref,
            usn: 0,
            timestamp: Timestamp::from_millis(0),
            reason,
            attributes: freespace_core::tree::FileAttributes::empty(),
            name: name.to_owned(),
        }
    }

    fn fixture() -> (tempfile::TempDir, FsTree, u32) {
        let directory = tempfile::tempdir().expect("temp dir");
        let mut tree = FsTree::new();
        let root = tree
            .add_node(
                None,
                NodeSpec {
                    name: paths::display(&paths::to_verbatim(directory.path()).expect("verbatim")),
                    kind: NodeKind::Directory,
                    file_ref: 5,
                    ..NodeSpec::default()
                },
            )
            .expect("root");

        (directory, tree, root)
    }

    #[test]
    fn adds_created_files_with_real_sizes() {
        let (directory, mut tree, root) = fixture();
        fs::write(directory.path().join("new.txt"), [b'a'; 12]).expect("file");

        let report = apply(
            &mut tree,
            &[record(42, 5, "new.txt", USN_REASON_FILE_CREATE)],
        );

        assert_eq!(report.added, 1);
        assert!(!report.requires_rescan);
        let child = tree.children(root).next().expect("child");
        assert_eq!(tree.name(child), "new.txt");
        assert_eq!(tree.node(child).size.logical, 12);
        assert_eq!(tree.node(root).size.logical, 12);
        assert_eq!(tree.node(root).file_count(), 1);
    }

    #[test]
    fn renames_and_updates_existing_nodes() {
        let (directory, mut tree, root) = fixture();
        fs::write(directory.path().join("after.txt"), [b'b'; 7]).expect("file");
        let node = tree
            .add_node(
                Some(root),
                NodeSpec {
                    name: "before.txt".to_owned(),
                    kind: NodeKind::File,
                    file_ref: 43,
                    size: SizeMetrics::file(1),
                    ..NodeSpec::default()
                },
            )
            .expect("node");

        let report = apply(
            &mut tree,
            &[record(43, 5, "after.txt", USN_REASON_RENAME_NEW_NAME)],
        );

        assert_eq!(report.changed, 1);
        assert_eq!(tree.name(node), "after.txt");
        assert_eq!(tree.node(node).size.logical, 7);
    }

    #[test]
    fn removes_deleted_subtrees() {
        let (_directory, mut tree, root) = fixture();
        let folder = tree
            .add_node(
                Some(root),
                NodeSpec {
                    name: "folder".to_owned(),
                    kind: NodeKind::Directory,
                    file_ref: 44,
                    ..NodeSpec::default()
                },
            )
            .expect("folder");
        tree.add_node(
            Some(folder),
            NodeSpec {
                name: "deep.bin".to_owned(),
                kind: NodeKind::File,
                file_ref: 45,
                size: SizeMetrics::file(9),
                ..NodeSpec::default()
            },
        )
        .expect("deep");
        tree.recompute_totals();

        let report = apply(
            &mut tree,
            &[record(44, 5, "folder", USN_REASON_FILE_DELETE)],
        );

        assert_eq!(report.removed, 2);
        assert!(tree.children(root).next().is_none());
        assert_eq!(tree.node(root).size.logical, 0);
    }

    #[test]
    fn requests_a_rescan_for_unknown_parents() {
        let (_directory, mut tree, _root) = fixture();

        let report = apply(
            &mut tree,
            &[record(50, 999, "orphan.txt", USN_REASON_FILE_CREATE)],
        );

        assert!(report.requires_rescan);
        assert_eq!(report.added, 0);
        assert_eq!(report.issues.len(), 1);
        assert!(report.issues[0].message.contains("full scan"));
    }

    #[test]
    fn ignores_unrelated_reasons() {
        let (_directory, mut tree, _root) = fixture();

        let report = apply(
            &mut tree,
            &[record(60, 5, "noop.txt", USN_REASON_DATA_EXTEND)],
        );

        assert_eq!(report.added, 0);
        assert_eq!(report.changed, 0);
    }
}
