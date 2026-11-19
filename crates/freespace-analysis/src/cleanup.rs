//! Stale-object detection by the most recent of the access and modification
//! times (FR-034, FR-035).
//!
//! Windows may stop updating the last-access time (the default on many systems
//! is an hourly or disabled update). The filter therefore compares the later of
//! the two timestamps and marks the entry when the access time is older than the
//! modification time: the caller must not present that date as reliable.

use std::path::PathBuf;

use freespace_core::{
    Timestamp,
    tree::{FsTree, NodeId, NodeKind},
};

/// Thresholds for the stale-file filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaleOptions {
    /// Objects not used for at least this many days are reported.
    pub older_than_days: u32,
    /// Include directories, not only files.
    pub include_directories: bool,
}

impl Default for StaleOptions {
    fn default() -> Self {
        Self {
            older_than_days: 180,
            include_directories: false,
        }
    }
}

/// One object that has not been used for longer than the threshold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaleEntry {
    /// Node inside the analysed tree.
    pub node_id: NodeId,
    /// Full path.
    pub path: PathBuf,
    /// Later of the access and modification times.
    pub last_used: Timestamp,
    /// Last modification time.
    pub modified: Timestamp,
    /// Last access time.
    pub accessed: Timestamp,
    /// `false` when the access date cannot be trusted.
    pub accessed_reliable: bool,
    /// Age in whole days.
    pub age_days: u32,
}

/// Combines the access and modification times and reports whether the access
/// date is trustworthy.
///
/// The access timestamp is treated as reliable when it is not older than the
/// modification timestamp and is not the Unix epoch (a zero value means the
/// file system never recorded it).
#[must_use]
pub fn newest_use(accessed: Timestamp, modified: Timestamp) -> (Timestamp, bool) {
    let reliable = accessed > Timestamp::UNIX_EPOCH && accessed >= modified;

    (accessed.max(modified), reliable)
}

/// Returns every file (and optionally directory) older than the threshold.
///
/// Results are ordered from the oldest object to the newest, so a cleanup screen
/// lists the safest candidates first.
#[must_use]
pub fn find_stale(tree: &FsTree, now: Timestamp, options: StaleOptions) -> Vec<StaleEntry> {
    let threshold =
        i64::from(options.older_than_days).saturating_mul(freespace_core::time::MILLIS_PER_DAY);
    let mut entries = Vec::new();
    let mut stack: Vec<NodeId> = tree.roots().collect();

    while let Some(id) = stack.pop() {
        let node = tree.node(id);
        if node.is_removed() {
            continue;
        }
        if node.kind.is_directory() {
            stack.extend(tree.children(id));
            if !options.include_directories || node.kind != NodeKind::Directory {
                continue;
            }
        } else if !matches!(node.kind, NodeKind::File | NodeKind::Hardlink) {
            continue;
        }

        let (last_used, accessed_reliable) =
            newest_use(node.timestamps.accessed, node.timestamps.modified);
        let age = now.as_millis().saturating_sub(last_used.as_millis());
        if age < threshold {
            continue;
        }
        let age_days =
            u32::try_from(age / freespace_core::time::MILLIS_PER_DAY).unwrap_or(u32::MAX);

        entries.push(StaleEntry {
            node_id: id,
            path: tree.path(id),
            last_used,
            modified: node.timestamps.modified,
            accessed: node.timestamps.accessed,
            accessed_reliable,
            age_days,
        });
    }

    entries.sort_by(|left, right| {
        left.last_used
            .cmp(&right.last_used)
            .then_with(|| left.path.cmp(&right.path))
    });

    entries
}

#[cfg(test)]
mod tests {
    use freespace_core::{
        Timestamp,
        size::SizeMetrics,
        time::MILLIS_PER_DAY,
        tree::{FsTree, NodeKind, NodeSpec, Timestamps},
    };

    use super::{StaleOptions, find_stale, newest_use};

    fn file(name: &str, accessed_days: i64, modified_days: i64) -> NodeSpec {
        let base = 1_700_000_000_000i64;
        NodeSpec {
            name: name.to_owned(),
            kind: NodeKind::File,
            size: SizeMetrics::file(10),
            timestamps: Timestamps {
                accessed: Timestamp::from_millis(base + accessed_days * MILLIS_PER_DAY),
                modified: Timestamp::from_millis(base + modified_days * MILLIS_PER_DAY),
                created: Timestamp::from_millis(base),
            },
            ..NodeSpec::default()
        }
    }

    fn fixture() -> FsTree {
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
        tree.add_node(Some(root), file("fresh.txt", 390, 10))
            .expect("fresh");
        tree.add_node(Some(root), file("stale.txt", 200, 100))
            .expect("stale");
        tree.add_node(Some(root), file("modified-only.txt", 1, 100))
            .expect("modified");
        tree.recompute_totals();

        tree
    }

    #[test]
    fn uses_the_latest_of_access_and_modification() {
        let (used, reliable) = newest_use(
            Timestamp::from_millis(10 * MILLIS_PER_DAY),
            Timestamp::from_millis(20 * MILLIS_PER_DAY),
        );

        assert_eq!(used.as_millis(), 20 * MILLIS_PER_DAY);
        assert!(!reliable, "an older access date is not trustworthy");

        let (used, reliable) = newest_use(
            Timestamp::from_millis(30 * MILLIS_PER_DAY),
            Timestamp::from_millis(20 * MILLIS_PER_DAY),
        );
        assert_eq!(used.as_millis(), 30 * MILLIS_PER_DAY);
        assert!(reliable);
    }

    #[test]
    fn reports_only_objects_older_than_the_threshold() {
        let tree = fixture();
        let now = Timestamp::from_millis(1_700_000_000_000 + 400 * MILLIS_PER_DAY);
        let stale = find_stale(&tree, now, StaleOptions::default());

        let names: Vec<&str> = stale
            .iter()
            .map(|entry| {
                entry
                    .path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default()
            })
            .collect();

        assert_eq!(names, ["modified-only.txt", "stale.txt"]);
        assert_eq!(stale[0].age_days, 300);
        assert!(!stale[0].accessed_reliable);
    }

    #[test]
    fn can_include_empty_directories() {
        let mut tree = fixture();
        let root = tree.roots().next().expect("root");
        tree.add_node(
            Some(root),
            NodeSpec {
                name: "old-folder".to_owned(),
                kind: NodeKind::Directory,
                ..NodeSpec::default()
            },
        )
        .expect("folder");
        tree.recompute_totals();

        let now = Timestamp::from_millis(1_700_000_000_000 + 400 * MILLIS_PER_DAY);
        let stale = find_stale(
            &tree,
            now,
            StaleOptions {
                older_than_days: 180,
                include_directories: true,
            },
        );

        assert!(
            stale.iter().any(|entry| entry.path.ends_with("old-folder")),
            "directories are included on request"
        );
    }
}
