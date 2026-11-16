//! Cleanup recommendations: temporary data, caches, logs, backups and stale
//! files with an explicit risk level (FR-036, FR-037).
//!
//! The classifier is deliberately conservative: it only recognises well-known
//! names and locations, never guesses from size alone, and never recommends the
//! analysed root or a link.

use std::path::{Path, PathBuf};

use freespace_core::{
    Timestamp,
    query::extension_of,
    tree::{FsTree, NodeId, NodeKind},
};

use crate::cleanup::{StaleOptions, find_stale, newest_use};

/// Category of a cleanup candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CleanupCategory {
    /// Temporary files.
    Temp,
    /// Application caches.
    Cache,
    /// Log files.
    Log,
    /// Backup copies.
    Backup,
    /// Objects that have not been used for a long time.
    Outdated,
}

impl CleanupCategory {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Temp => "temp",
            Self::Cache => "cache",
            Self::Log => "log",
            Self::Backup => "backup",
            Self::Outdated => "outdated",
        }
    }
}

/// Risk of removing the candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CleanupRisk {
    /// Safe to remove: the system or an application recreates the data.
    Low,
    /// Review the object first.
    Medium,
    /// Removing it can lose user data.
    High,
}

impl CleanupRisk {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

/// Recommended action for a candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CleanupAction {
    /// Show it to the user and let them decide.
    Review,
    /// Offer deletion directly.
    Delete,
    /// Keep it; listed for information only.
    Keep,
}

impl CleanupAction {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Review => "review",
            Self::Delete => "delete",
            Self::Keep => "keep",
        }
    }
}

/// One recommendation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupCandidate {
    /// Node inside the analysed tree.
    pub node_id: NodeId,
    /// Full path.
    pub path: PathBuf,
    /// Category.
    pub category: CleanupCategory,
    /// Human-readable explanation.
    pub reason: String,
    /// Bytes that removal would free.
    pub reclaimable_bytes: u64,
    /// Risk of removal.
    pub risk: CleanupRisk,
    /// Recommended action.
    pub recommended_action: CleanupAction,
}

/// Options for [`recommend`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecommendOptions {
    /// Reference instant for stale detection.
    pub now: Timestamp,
    /// Report stale files older than this many days; zero disables the check.
    pub older_than_days: u32,
}

impl Default for RecommendOptions {
    fn default() -> Self {
        Self {
            now: Timestamp::now(),
            older_than_days: 0,
        }
    }
}

/// Classifies a file from its path and name.
#[must_use]
pub fn classify_path(path: &Path) -> Option<(CleanupCategory, CleanupRisk, &'static str)> {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let text = path
        .to_string_lossy()
        .to_ascii_lowercase()
        .replace('/', "\\");
    let extension = extension_of(&name).to_ascii_lowercase();

    if matches!(
        extension.as_str(),
        "tmp" | "temp" | "~tmp" | "swp" | "part" | "crdownload"
    ) {
        return Some((CleanupCategory::Temp, CleanupRisk::Low, "temporary file"));
    }
    if matches!(extension.as_str(), "log" | "etl" | "evtx" | "dmp" | "mdmp") {
        return Some((
            CleanupCategory::Log,
            CleanupRisk::Medium,
            "log or diagnostic file",
        ));
    }
    if matches!(
        extension.as_str(),
        "bak" | "backup" | "old" | "orig" | "sav" | "wbk"
    ) {
        return Some((CleanupCategory::Backup, CleanupRisk::High, "backup copy"));
    }
    if has_component(&text, &["cache", "caches", "cache2", "shadercache"]) {
        return Some((
            CleanupCategory::Cache,
            CleanupRisk::Low,
            "application cache",
        ));
    }
    if has_component(&text, &["temp", "tmp", "temporary"]) {
        return Some((
            CleanupCategory::Temp,
            CleanupRisk::Low,
            "temporary directory",
        ));
    }
    if has_component(&text, &["logs", "log"]) {
        return Some((CleanupCategory::Log, CleanupRisk::Medium, "log directory"));
    }
    if has_component(&text, &["backup", "backups", "backup-old"]) {
        return Some((
            CleanupCategory::Backup,
            CleanupRisk::High,
            "backup directory",
        ));
    }

    None
}

fn has_component(path: &str, names: &[&str]) -> bool {
    path.split('\\').any(|component| names.contains(&component))
}

/// Builds recommendations for the whole tree.
///
/// A directory that matches a category is reported as one candidate and its
/// children are not reported twice.
#[must_use]
pub fn recommend(tree: &FsTree, options: RecommendOptions) -> Vec<CleanupCandidate> {
    let mut candidates = Vec::new();
    let mut stack: Vec<NodeId> = tree.roots().collect();

    while let Some(id) = stack.pop() {
        let node = tree.node(id);
        if node.is_removed() || node.kind.is_link() {
            continue;
        }
        if node.kind.is_directory() {
            if tree.node(id).parent().is_none() {
                stack.extend(tree.children(id));
                continue;
            }
            match classify_path(&tree.path(id)) {
                Some((category, risk, reason)) => candidates.push(CleanupCandidate {
                    node_id: id,
                    path: tree.path(id),
                    category,
                    reason: reason.to_owned(),
                    reclaimable_bytes: node.size.logical,
                    risk,
                    recommended_action: action_for(risk),
                }),
                None => stack.extend(tree.children(id)),
            }
        } else if matches!(node.kind, NodeKind::File | NodeKind::Hardlink) {
            if let Some((category, risk, reason)) = classify_path(&tree.path(id)) {
                candidates.push(CleanupCandidate {
                    node_id: id,
                    path: tree.path(id),
                    category,
                    reason: reason.to_owned(),
                    reclaimable_bytes: node.size.logical,
                    risk,
                    recommended_action: action_for(risk),
                });
            }
        }
    }

    if options.older_than_days > 0 {
        let reported: Vec<PathBuf> = candidates.iter().map(|item| item.path.clone()).collect();
        for entry in find_stale(
            tree,
            options.now,
            StaleOptions {
                older_than_days: options.older_than_days,
                include_directories: false,
            },
        ) {
            if reported
                .iter()
                .any(|path| path.starts_with(&entry.path) || entry.path.starts_with(path))
            {
                continue;
            }
            let (_, reliable) = newest_use(entry.accessed, entry.modified);
            candidates.push(CleanupCandidate {
                node_id: entry.node_id,
                path: entry.path,
                category: CleanupCategory::Outdated,
                reason: if reliable {
                    format!("not used for {} days", entry.age_days)
                } else {
                    format!(
                        "modified {} days ago; the access date is unreliable",
                        entry.age_days
                    )
                },
                reclaimable_bytes: tree.node(entry.node_id).size.logical,
                risk: CleanupRisk::Medium,
                recommended_action: CleanupAction::Review,
            });
        }
    }

    candidates.sort_by(|left, right| {
        left.risk
            .cmp(&right.risk)
            .then_with(|| right.reclaimable_bytes.cmp(&left.reclaimable_bytes))
            .then_with(|| left.path.cmp(&right.path))
    });

    candidates
}

const fn action_for(risk: CleanupRisk) -> CleanupAction {
    match risk {
        CleanupRisk::Low => CleanupAction::Delete,
        CleanupRisk::Medium | CleanupRisk::High => CleanupAction::Review,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use freespace_core::{
        Timestamp,
        size::SizeMetrics,
        time::MILLIS_PER_DAY,
        tree::{FsTree, NodeKind, NodeSpec, Timestamps},
    };

    use super::{
        CleanupAction, CleanupCategory, CleanupRisk, RecommendOptions, classify_path, recommend,
    };

    #[test]
    fn classifies_well_known_names() {
        assert_eq!(
            classify_path(Path::new(r"C:\data\report.tmp")).map(|item| item.0),
            Some(CleanupCategory::Temp)
        );
        assert_eq!(
            classify_path(Path::new(r"C:\AppData\Local\app\Cache\blob.bin")).map(|item| item.0),
            Some(CleanupCategory::Cache)
        );
        assert_eq!(
            classify_path(Path::new(r"C:\data\archive.log")).map(|item| item.0),
            Some(CleanupCategory::Log)
        );
        assert_eq!(
            classify_path(Path::new(r"C:\data\setup.bak")).map(|item| item.0),
            Some(CleanupCategory::Backup)
        );
        assert_eq!(classify_path(Path::new(r"C:\data\photo.jpg")), None);
        assert_eq!(classify_path(Path::new(r"C:\data\README")), None);
    }

    #[test]
    fn recommends_directories_once_and_keeps_low_risk_first() {
        let mut tree = FsTree::new();
        let root = tree
            .add_node(
                None,
                NodeSpec {
                    name: "root".to_owned(),
                    kind: NodeKind::Directory,
                    ..NodeSpec::default()
                },
            )
            .expect("root");
        let cache = tree
            .add_node(
                Some(root),
                NodeSpec {
                    name: "Cache".to_owned(),
                    kind: NodeKind::Directory,
                    ..NodeSpec::default()
                },
            )
            .expect("cache");
        tree.add_node(
            Some(cache),
            NodeSpec {
                name: "blob.bin".to_owned(),
                kind: NodeKind::File,
                size: SizeMetrics::file(500),
                ..NodeSpec::default()
            },
        )
        .expect("blob");
        tree.add_node(
            Some(root),
            NodeSpec {
                name: "report.bak".to_owned(),
                kind: NodeKind::File,
                size: SizeMetrics::file(1_000),
                ..NodeSpec::default()
            },
        )
        .expect("backup");
        tree.add_node(
            Some(root),
            NodeSpec {
                name: "photo.jpg".to_owned(),
                kind: NodeKind::File,
                size: SizeMetrics::file(2_000),
                ..NodeSpec::default()
            },
        )
        .expect("photo");
        tree.recompute_totals();

        let candidates = recommend(&tree, RecommendOptions::default());

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].category, CleanupCategory::Cache);
        assert_eq!(candidates[0].reclaimable_bytes, 500);
        assert_eq!(candidates[0].recommended_action, CleanupAction::Delete);
        assert_eq!(candidates[1].category, CleanupCategory::Backup);
        assert_eq!(candidates[1].risk, CleanupRisk::High);
        assert_eq!(candidates[1].recommended_action, CleanupAction::Review);
    }

    #[test]
    fn flags_stale_files_without_duplicating_candidates() {
        let base = 1_700_000_000_000i64;
        let mut tree = FsTree::new();
        let root = tree
            .add_node(
                None,
                NodeSpec {
                    name: "root".to_owned(),
                    kind: NodeKind::Directory,
                    ..NodeSpec::default()
                },
            )
            .expect("root");
        tree.add_node(
            Some(root),
            NodeSpec {
                name: "old.dat".to_owned(),
                kind: NodeKind::File,
                size: SizeMetrics::file(100),
                timestamps: Timestamps {
                    accessed: Timestamp::from_millis(base),
                    modified: Timestamp::from_millis(base),
                    created: Timestamp::from_millis(base),
                },
                ..NodeSpec::default()
            },
        )
        .expect("old");
        tree.add_node(
            Some(root),
            NodeSpec {
                name: "old.log".to_owned(),
                kind: NodeKind::File,
                size: SizeMetrics::file(50),
                timestamps: Timestamps {
                    accessed: Timestamp::from_millis(base),
                    modified: Timestamp::from_millis(base),
                    created: Timestamp::from_millis(base),
                },
                ..NodeSpec::default()
            },
        )
        .expect("log");
        tree.recompute_totals();

        let candidates = recommend(
            &tree,
            RecommendOptions {
                now: Timestamp::from_millis(base + 400 * MILLIS_PER_DAY),
                older_than_days: 180,
            },
        );

        assert_eq!(
            candidates
                .iter()
                .filter(|item| item.category == CleanupCategory::Outdated)
                .count(),
            1,
            "the log keeps its more specific category"
        );
        assert!(candidates.iter().any(|item| item.path.ends_with("old.dat")));
    }
}
