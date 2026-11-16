//! Snapshot comparison (FR-039…FR-041).
//!
//! Both snapshots are reduced to a map of relative path → size and modification
//! time, then merged. The result is sorted so the largest regressions are first:
//! a snapshot diff is a tool for finding what grew, not a raw log.

use std::collections::{BTreeMap, BTreeSet};

use freespace_core::{Error, Result};

use crate::snapshot::{SnapshotPayload, is_directory};

/// Kind of change between two snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiffChange {
    /// The object exists only in the target.
    Added,
    /// The object exists only in the base.
    Removed,
    /// The object exists in both but differs in size or modification time.
    Changed,
}

impl DiffChange {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::Changed => "changed",
        }
    }
}

/// One entry of the file-level diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffEntry {
    /// Path relative to the snapshot root.
    pub path: String,
    /// Kind of change.
    pub change: DiffChange,
    /// Whether the object is a directory in either snapshot.
    pub is_directory: bool,
    /// Signed change of the physical size.
    pub size_delta: i64,
    /// Signed change of the modification time in seconds.
    pub mtime_delta: i64,
    /// Physical size in the base snapshot.
    pub base_size: u64,
    /// Physical size in the target snapshot.
    pub target_size: u64,
}

/// Growth of one directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrowthEntry {
    /// Path relative to the snapshot root.
    pub path: String,
    /// Signed growth of the subtree.
    pub delta: i64,
}

/// Complete comparison result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotDiff {
    /// Volume identifier shared by both snapshots.
    pub volume_id: String,
    /// Creation instant of the base snapshot.
    pub base_created_at: String,
    /// Creation instant of the target snapshot.
    pub target_created_at: String,
    /// New objects.
    pub added: Vec<DiffEntry>,
    /// Removed objects.
    pub removed: Vec<DiffEntry>,
    /// Changed objects.
    pub changed: Vec<DiffEntry>,
    /// Growth per directory, largest first.
    pub growth_by_directory: Vec<GrowthEntry>,
    /// Total signed growth of the analysed tree.
    pub total_growth_bytes: i64,
}

impl SnapshotDiff {
    /// Number of file-level changes.
    #[must_use]
    pub fn change_count(&self) -> usize {
        self.added.len() + self.removed.len() + self.changed.len()
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct NodeSummary {
    physical: u64,
    mtime: i64,
    is_directory: bool,
}

/// Compares two snapshots of the same volume.
///
/// # Errors
/// Returns [`Error::CorruptedSnapshot`] when a checksum is invalid and
/// [`Error::IncompatibleSnapshot`] when the snapshots describe different
/// volumes.
pub fn compare(base: &SnapshotPayload, target: &SnapshotPayload) -> Result<SnapshotDiff> {
    base.verify()?;
    target.verify()?;

    if base.volume.id != target.volume.id {
        return Err(Error::IncompatibleSnapshot(format!(
            "base snapshot is from volume `{}`, target from `{}`",
            base.volume.mount_point, target.volume.mount_point
        )));
    }

    let base_nodes = index(base);
    let target_nodes = index(target);
    let keys: BTreeSet<&String> = base_nodes.keys().chain(target_nodes.keys()).collect();
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();
    let mut total_growth_bytes = 0i64;

    for key in keys {
        match (base_nodes.get(key), target_nodes.get(key)) {
            (None, Some(target_node)) => {
                let delta = to_i64(target_node.physical);
                if !target_node.is_directory {
                    total_growth_bytes = total_growth_bytes.saturating_add(delta);
                }
                added.push(entry(
                    key,
                    DiffChange::Added,
                    NodeSummary::default(),
                    *target_node,
                    delta,
                ));
            }
            (Some(base_node), None) => {
                let delta = to_i64(base_node.physical);
                if !base_node.is_directory {
                    total_growth_bytes = total_growth_bytes.saturating_sub(delta);
                }
                removed.push(entry(
                    key,
                    DiffChange::Removed,
                    *base_node,
                    NodeSummary::default(),
                    -delta,
                ));
            }
            (Some(base_node), Some(target_node)) => {
                if base_node.physical == target_node.physical
                    && base_node.mtime == target_node.mtime
                    && base_node.is_directory == target_node.is_directory
                {
                    continue;
                }
                let delta = to_i64(target_node.physical).saturating_sub(to_i64(base_node.physical));
                if !base_node.is_directory && !target_node.is_directory {
                    total_growth_bytes = total_growth_bytes.saturating_add(delta);
                }
                changed.push(entry(
                    key,
                    DiffChange::Changed,
                    *base_node,
                    *target_node,
                    delta,
                ));
            }
            (None, None) => {}
        }
    }

    sort_entries(&mut added);
    sort_entries(&mut removed);
    sort_entries(&mut changed);

    let growth_by_directory = grow_by_directory(&base_nodes, &target_nodes);

    Ok(SnapshotDiff {
        volume_id: base.volume.id.clone(),
        base_created_at: base.created_at.clone(),
        target_created_at: target.created_at.clone(),
        added,
        removed,
        changed,
        growth_by_directory,
        total_growth_bytes,
    })
}

fn entry(
    path: &str,
    change: DiffChange,
    base_node: NodeSummary,
    target_node: NodeSummary,
    size_delta: i64,
) -> DiffEntry {
    DiffEntry {
        path: path.to_owned(),
        change,
        is_directory: base_node.is_directory || target_node.is_directory,
        size_delta,
        mtime_delta: if change == DiffChange::Changed {
            target_node.mtime.saturating_sub(base_node.mtime)
        } else {
            0
        },
        base_size: base_node.physical,
        target_size: target_node.physical,
    }
}

fn index(payload: &SnapshotPayload) -> BTreeMap<String, NodeSummary> {
    let mut map = BTreeMap::new();

    for (index, path) in payload.relative_paths() {
        map.insert(
            path,
            NodeSummary {
                physical: payload.nodes.physical_size.get(index).copied().unwrap_or(0),
                mtime: payload.nodes.mtime.get(index).copied().unwrap_or(0),
                is_directory: is_directory(&payload.nodes, index),
            },
        );
    }

    map
}

fn sort_entries(entries: &mut [DiffEntry]) {
    entries.sort_by(|left, right| {
        right
            .size_delta
            .unsigned_abs()
            .cmp(&left.size_delta.unsigned_abs())
            .then_with(|| left.path.cmp(&right.path))
    });
}

fn grow_by_directory(
    base: &BTreeMap<String, NodeSummary>,
    target: &BTreeMap<String, NodeSummary>,
) -> Vec<GrowthEntry> {
    let mut directories: BTreeSet<&String> = BTreeSet::new();
    for (path, node) in base.iter().chain(target.iter()) {
        if node.is_directory {
            directories.insert(path);
        }
    }

    let mut growth: BTreeMap<String, i64> = BTreeMap::new();
    let keys: BTreeSet<&String> = base.keys().chain(target.keys()).collect();
    for path in keys {
        // A directory's own size is the sum of its descendants; counting it as
        // well as the children would double every subtree change.
        if base.get(path).is_some_and(|summary| summary.is_directory)
            || target.get(path).is_some_and(|summary| summary.is_directory)
        {
            continue;
        }
        let base_size = base.get(path).map_or(0, |summary| summary.physical);
        let target_size = target.get(path).map_or(0, |summary| summary.physical);
        let delta = to_i64(target_size).saturating_sub(to_i64(base_size));
        if delta == 0 {
            continue;
        }
        let components: Vec<&str> = path.split('\\').filter(|part| !part.is_empty()).collect();
        for end in 1..=components.len() {
            let prefix = components[..end].join("\\");
            if directories.contains(&prefix) {
                *growth.entry(prefix).or_insert(0) += delta;
            }
        }
    }

    let mut entries: Vec<GrowthEntry> = growth
        .into_iter()
        .map(|(path, delta)| GrowthEntry { path, delta })
        .collect();
    entries.sort_by(|left, right| {
        right
            .delta
            .cmp(&left.delta)
            .then_with(|| left.path.cmp(&right.path))
    });

    entries
}

fn to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use freespace_core::{
        Timestamp,
        model::{FileSystem, Volume, VolumeStatus},
        size::SizeMetrics,
        tree::{FsTree, NodeKind, NodeSpec, Timestamps},
    };

    use super::{DiffChange, compare};
    use crate::snapshot::{SnapshotPayload, SnapshotSettings, from_tree};

    fn volume() -> Volume {
        Volume {
            id: Volume::id_for_mount_point(PathBuf::from(r"C:\").as_path()),
            mount_point: PathBuf::from(r"C:\"),
            label: "System".to_owned(),
            file_system: FileSystem::Ntfs,
            total_bytes: 1_000_000,
            free_bytes: 500_000,
            cluster_size: Some(4_096),
            scan_method: None,
            is_elevated: false,
            is_removable: false,
            status: VolumeStatus::Available,
        }
    }

    fn snapshot(files: &[(&str, u64, i64)]) -> SnapshotPayload {
        let mut tree = FsTree::new();
        let root = tree
            .add_node(
                None,
                NodeSpec {
                    name: r"C:\data".to_owned(),
                    kind: NodeKind::Directory,
                    ..NodeSpec::default()
                },
            )
            .expect("root");
        let nested = tree
            .add_node(
                Some(root),
                NodeSpec {
                    name: "nested".to_owned(),
                    kind: NodeKind::Directory,
                    ..NodeSpec::default()
                },
            )
            .expect("nested");
        for (path, size, mtime) in files {
            let (parent, name) = if let Some(name) = path.strip_prefix("nested/") {
                (nested, name)
            } else {
                (root, *path)
            };
            tree.add_node(
                Some(parent),
                NodeSpec {
                    name: name.to_owned(),
                    kind: NodeKind::File,
                    size: SizeMetrics::file(*size),
                    timestamps: Timestamps {
                        modified: Timestamp::from_millis(*mtime * 1_000),
                        ..Timestamps::default()
                    },
                    ..NodeSpec::default()
                },
            )
            .expect("file");
        }
        tree.recompute_totals();

        from_tree(
            &tree,
            &volume(),
            &[PathBuf::from(r"C:\data")],
            SnapshotSettings::default(),
            Timestamp::from_millis(1_700_000_000_000),
        )
        .expect("snapshot")
    }

    #[test]
    fn reports_added_removed_and_changed() {
        let base = snapshot(&[("a.txt", 100, 10), ("nested/b.bin", 200, 10)]);
        let target = snapshot(&[("a.txt", 150, 20), ("nested/c.bin", 300, 20)]);

        let diff = compare(&base, &target).expect("compare");

        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.added[0].path, r"nested\c.bin");
        assert_eq!(diff.added[0].size_delta, 300);
        assert_eq!(diff.removed.len(), 1);
        assert_eq!(diff.removed[0].path, r"nested\b.bin");
        assert_eq!(diff.changed.len(), 2, "the nested directory grew too");
        let changed_file = diff
            .changed
            .iter()
            .find(|entry| entry.path == "a.txt")
            .expect("changed file");
        assert_eq!(changed_file.size_delta, 50);
        assert_eq!(changed_file.change, DiffChange::Changed);
        assert_eq!(diff.total_growth_bytes, 150);
        assert_eq!(diff.change_count(), 4);
    }

    #[test]
    fn sorts_growth_by_the_largest_regression() {
        let base = snapshot(&[("nested/b.bin", 200, 10)]);
        let target = snapshot(&[("nested/a.bin", 100, 10), ("nested/c.bin", 700, 10)]);

        let diff = compare(&base, &target).expect("compare");
        let nested = diff
            .growth_by_directory
            .iter()
            .find(|entry| entry.path == "nested")
            .expect("nested growth");

        assert_eq!(nested.delta, 600);
        assert_eq!(diff.total_growth_bytes, 600);
    }

    #[test]
    fn rejects_snapshot_volume_mismatch() {
        let base = snapshot(&[("a.txt", 100, 10)]);
        let mut target = snapshot(&[("a.txt", 100, 10)]);
        target.volume.id = freespace_core::Uuid::new_v4().to_string();
        target.checksum = target.compute_checksum().expect("checksum");

        let error = compare(&base, &target).expect_err("different volumes");

        assert_eq!(error.code().as_str(), "E_INCOMPATIBLE_SNAPSHOT");
    }
}
