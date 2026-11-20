//! Exact size details for one node (US4, T098).
//!
//! The screen answers "why does this object occupy the space it does": logical
//! length, allocation, the hard-link contribution, alternate data streams and
//! the savings of compression and sparse regions. Every number is measured
//! through [`freespace_platform::platform`]; Windows adds attribute and reparse
//! details, other platforms derive them from the portable metadata subset.
//!
//! Sizes of a scanned node come from the immutable snapshot. When the object
//! still exists, the live values replace them so the screen can be trusted right
//! after a scan; when it is gone, the snapshot values are returned with
//! `estimated = true` instead of an error, because a stale scan is not a failure.

use std::{fs, path::Path};

use freespace_core::{
    size::SizeMetrics,
    tree::{FileAttributes, NodeId},
};
use freespace_platform::{paths, platform};

use crate::{
    dto::{AttributeFlags, NodeKind, SizeDetails},
    error::{UiError, UiErrorCode},
    tree::TreeSnapshot,
};

/// Localisation key of the physical metric, used when hard links reduce the
/// contribution of an object.
pub const METRIC_PHYSICAL: &str = "size_details.metric.physical";
/// Localisation key of the on-disk metric, the default for ordinary objects.
pub const METRIC_ON_DISK: &str = "size_details.metric.on_disk";

/// Facts measured directly from the file system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LiveFacts {
    logical: u64,
    allocated: u64,
    ads: u64,
    attribute_mask: u32,
}

impl LiveFacts {
    fn compressed(self) -> bool {
        self.attribute_mask & FileAttributes::COMPRESSED.bits() != 0
    }

    fn sparse(self) -> bool {
        self.attribute_mask & FileAttributes::SPARSE_FILE.bits() != 0
    }
}

/// Returns exact details of one node of the current generation.
///
/// # Errors
/// Returns `E_STALE_NODE` when the identifier does not belong to the snapshot
/// generation.
pub fn size_details(snapshot: &TreeSnapshot, node_id: NodeId) -> Result<SizeDetails, UiError> {
    let row = snapshot.node_row(node_id)?;
    let mut details = SizeDetails {
        path: row.path.clone(),
        name: row.name.clone(),
        logical_size: row.logical_size,
        on_disk_size: row.on_disk_size,
        physical_size: row.physical_size,
        allocated_size: row.on_disk_size,
        hardlink_contribution: row.logical_size.saturating_sub(row.physical_size),
        ads_size: row.ads_size,
        compression_savings: 0,
        sparse_savings: 0,
        link_target: row.link_target.clone(),
        metric_label: METRIC_ON_DISK.to_owned(),
        attributes: row.attributes,
        estimated: true,
    };

    if let Some(facts) = measure(Path::new(&row.path)) {
        let directory = matches!(row.kind, NodeKind::Directory);
        let reparse_target = link_target(Path::new(&row.path)).or(row.link_target);

        // Directory aggregates already include every child, so the live
        // metadata of the directory entry itself must not replace them.
        if !directory {
            details.logical_size = facts.logical;
            details.on_disk_size = facts.allocated;
            details.allocated_size = facts.allocated;
            details.ads_size = facts.ads;
            // The scanner marks an additional hard link with a zero physical
            // size; the live query cannot count links without WinAPI calls that
            // do not belong in this crate, so the scan value is kept.
            details.physical_size = if row.physical_size == 0 && row.logical_size > 0 {
                0
            } else {
                facts.logical
            };
            details.estimated = false;
        } else {
            details.allocated_size = facts.allocated.max(details.on_disk_size);
            details.estimated = false;
        }

        details.hardlink_contribution = details.logical_size.saturating_sub(details.physical_size);
        details.link_target = reparse_target;
        details.attributes = flags(facts.attribute_mask);
        details.compression_savings = compression_savings(
            details.logical_size,
            details.allocated_size,
            facts.compressed(),
            facts.sparse(),
        );
        details.sparse_savings =
            sparse_savings(details.logical_size, details.allocated_size, facts.sparse());
    }

    details.metric_label = metric_label(details.hardlink_contribution).to_owned();

    Ok(details)
}

/// Inspects one existing path, without a scan snapshot.
///
/// # Errors
/// Returns `E_VALIDATION` for a malformed path and `E_NOT_FOUND` when the object
/// is missing or its metadata cannot be read.
pub fn inspect_path(path: &Path) -> Result<SizeDetails, UiError> {
    let normalized = paths::normalize(path)
        .map_err(|error| UiError::validation(format!("`{}`: {error}", paths::display(path))))?;
    let facts = measure(&normalized)
        .ok_or_else(|| UiError::new(UiErrorCode::NotFound).with_details(paths::display(path)))?;
    let attributes = flags(facts.attribute_mask);
    let link_target = link_target(&normalized);
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| paths::display(&normalized));
    let physical = facts.logical;
    let hardlink_contribution = facts.logical.saturating_sub(physical);

    Ok(SizeDetails {
        path: paths::display(&normalized),
        name,
        logical_size: facts.logical,
        on_disk_size: facts.allocated,
        physical_size: physical,
        allocated_size: facts.allocated,
        hardlink_contribution,
        ads_size: facts.ads,
        compression_savings: compression_savings(
            facts.logical,
            facts.allocated,
            facts.compressed(),
            facts.sparse(),
        ),
        sparse_savings: sparse_savings(facts.logical, facts.allocated, facts.sparse()),
        link_target,
        metric_label: metric_label(hardlink_contribution).to_owned(),
        attributes,
        estimated: false,
    })
}

/// Converts a raw Windows attribute mask into the interface flags.
#[must_use]
pub fn flags(mask: u32) -> AttributeFlags {
    let attributes = FileAttributes::from_windows_attributes(mask);
    AttributeFlags {
        read_only: attributes.contains(FileAttributes::READ_ONLY),
        hidden: attributes.contains(FileAttributes::HIDDEN),
        system: attributes.contains(FileAttributes::SYSTEM),
        compressed: attributes.contains(FileAttributes::COMPRESSED),
        sparse: attributes.contains(FileAttributes::SPARSE_FILE),
        reparse: attributes.contains(FileAttributes::REPARSE_POINT),
    }
}

/// Bytes saved by compression.
///
/// Sparse regions win when both flags are set, because a sparse file is usually
/// also compressed and attributing the same bytes twice would inflate the total.
#[must_use]
pub fn compression_savings(logical: u64, allocated: u64, compressed: bool, sparse: bool) -> u64 {
    if compressed && !sparse {
        logical.saturating_sub(allocated)
    } else {
        0
    }
}

/// Bytes saved by sparse regions.
#[must_use]
pub fn sparse_savings(logical: u64, allocated: u64, sparse: bool) -> u64 {
    if sparse {
        logical.saturating_sub(allocated)
    } else {
        0
    }
}

/// Localisation key naming the metric that best describes an object.
#[must_use]
pub fn metric_label(hardlink_contribution: u64) -> &'static str {
    if hardlink_contribution > 0 {
        METRIC_PHYSICAL
    } else {
        METRIC_ON_DISK
    }
}

/// Measures one path; returns `None` when it no longer exists or cannot be read.
fn measure(path: &Path) -> Option<LiveFacts> {
    let normalized = paths::normalize(path).ok()?;
    let metadata = fs::symlink_metadata(&normalized).ok()?;
    let logical = metadata.len();
    let allocated = platform()
        .metadata
        .allocated_size(&normalized)
        .unwrap_or(logical);
    let ads = if metadata.is_dir() {
        0
    } else {
        platform()
            .metadata
            .streams(&normalized)
            .map(|streams| streams.iter().map(|stream| stream.size).sum())
            .unwrap_or(0)
    };
    let attribute_mask = platform()
        .metadata
        .attributes(&normalized)
        .unwrap_or_default();

    Some(LiveFacts {
        logical,
        allocated,
        ads,
        attribute_mask,
    })
}

/// Returns the target of a symbolic link or junction as a display string.
fn link_target(path: &Path) -> Option<String> {
    #[cfg(windows)]
    {
        freespace_platform::reparse::inspect(path)
            .ok()
            .flatten()
            .and_then(|info| info.target)
            .map(|target| paths::display(&target))
    }
    #[cfg(not(windows))]
    {
        platform().metadata.link_kind(path).ok().flatten()?;
        fs::read_link(path)
            .ok()
            .map(|target| paths::display(&target))
    }
}

/// Returns the size metrics of a node as the scanner recorded them.
///
/// Kept for callers that compare the snapshot against the live values; the
/// screen itself uses [`size_details`].
#[must_use]
pub fn snapshot_metrics(snapshot: &TreeSnapshot, node_id: NodeId) -> Option<SizeMetrics> {
    let index = usize::try_from(node_id).ok()?;
    if index >= snapshot.tree().len() {
        return None;
    }
    let node = snapshot.tree().node(node_id);

    (!node.is_removed()).then_some(node.size)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use freespace_core::{
        model::{FileSystem, Volume, VolumeStatus},
        size::SizeMetrics,
        tree::{FileAttributes, FsTree, NodeKind as CoreKind, NodeSpec},
    };

    use super::{
        METRIC_ON_DISK, METRIC_PHYSICAL, compression_savings, flags, inspect_path, size_details,
        sparse_savings,
    };
    use crate::{dto::NodeKind, error::UiErrorCode, tree::TreeSnapshot};

    fn volume() -> Volume {
        Volume {
            id: Volume::id_for_mount_point(std::path::Path::new(r"C:\")),
            mount_point: r"C:\".into(),
            label: "Test".to_owned(),
            file_system: FileSystem::Ntfs,
            total_bytes: 1_000,
            free_bytes: 100,
            cluster_size: Some(4_096),
            scan_method: None,
            is_elevated: false,
            is_removable: false,
            status: VolumeStatus::Available,
        }
    }

    fn snapshot_with(
        path: &std::path::Path,
        size: SizeMetrics,
        attributes: FileAttributes,
    ) -> TreeSnapshot {
        let mut tree = FsTree::new();
        let root = tree
            .add_node(
                None,
                NodeSpec {
                    name: path
                        .parent()
                        .map(|parent| parent.to_string_lossy().into_owned())
                        .unwrap_or_else(|| r"C:\".to_owned()),
                    kind: CoreKind::Directory,
                    ..NodeSpec::default()
                },
            )
            .expect("root");
        tree.add_node(
            Some(root),
            NodeSpec {
                name: path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "file.bin".to_owned()),
                kind: CoreKind::File,
                size,
                attributes,
                ..NodeSpec::default()
            },
        )
        .expect("file");
        tree.recompute_totals();

        TreeSnapshot::new(tree, 4, volume(), None)
    }

    #[test]
    fn measures_a_live_file_and_marks_it_exact() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("payload.bin");
        std::fs::write(&path, vec![0xAB; 8_192]).expect("write");
        let snapshot = snapshot_with(&path, SizeMetrics::file(8_192), FileAttributes::empty());

        let details = size_details(&snapshot, 1).expect("details");

        assert_eq!(details.logical_size, 8_192);
        assert!(details.allocated_size >= 8_192);
        assert_eq!(details.ads_size, 0);
        assert_eq!(details.hardlink_contribution, 0);
        assert_eq!(details.metric_label, METRIC_ON_DISK);
        assert!(!details.estimated);
        assert_eq!(details.attributes, crate::dto::AttributeFlags::default());
    }

    #[test]
    fn falls_back_to_snapshot_values_for_a_missing_object() {
        let path = PathBuf::from(r"C:\definitely\missing\object.bin");
        let snapshot = snapshot_with(
            &path,
            SizeMetrics::exact(1_000, 512, 1_000, 300),
            FileAttributes::empty(),
        );

        let details = size_details(&snapshot, 1).expect("details");

        assert_eq!(details.logical_size, 1_000);
        assert_eq!(details.on_disk_size, 512);
        assert_eq!(details.physical_size, 1_000);
        assert_eq!(details.ads_size, 300);
        assert_eq!(details.hardlink_contribution, 0);
        assert!(details.estimated);
    }

    #[test]
    fn reports_a_hardlink_contribution_from_the_physical_size() {
        let path = PathBuf::from(r"C:\definitely\missing\link.bin");
        let snapshot = snapshot_with(
            &path,
            SizeMetrics::exact(4_096, 4_096, 0, 0),
            FileAttributes::empty(),
        );

        let details = size_details(&snapshot, 1).expect("details");

        assert_eq!(details.hardlink_contribution, 4_096);
        assert_eq!(details.physical_size, 0);
        assert_eq!(details.metric_label, METRIC_PHYSICAL);
    }

    #[test]
    fn rejects_nodes_from_another_generation() {
        let path = PathBuf::from(r"C:\missing.bin");
        let snapshot = snapshot_with(&path, SizeMetrics::file(1), FileAttributes::empty());

        let error = size_details(&snapshot, 99).expect_err("stale");
        assert_eq!(error.code, UiErrorCode::StaleNode);
    }

    #[test]
    fn splits_compression_and_sparse_savings_without_double_counting() {
        assert_eq!(compression_savings(10_000, 2_000, true, false), 8_000);
        assert_eq!(compression_savings(10_000, 2_000, false, false), 0);
        assert_eq!(compression_savings(10_000, 2_000, true, true), 0);
        assert_eq!(sparse_savings(10_000, 2_000, true), 8_000);
        assert_eq!(sparse_savings(10_000, 12_000, true), 0);
    }

    #[test]
    fn decodes_attribute_flags() {
        let flags = flags(
            FileAttributes::READ_ONLY.bits()
                | FileAttributes::HIDDEN.bits()
                | FileAttributes::COMPRESSED.bits(),
        );

        assert!(flags.read_only);
        assert!(flags.hidden);
        assert!(flags.compressed);
        assert!(!flags.sparse);
        assert!(!flags.system);
    }

    #[test]
    fn inspects_a_path_directly() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("plain.txt");
        std::fs::write(&path, b"payload").expect("write");

        let details = inspect_path(&path).expect("details");

        assert_eq!(details.logical_size, 7);
        assert_eq!(details.physical_size, 7);
        assert!(!details.estimated);
        assert_eq!(details.name, "plain.txt");
    }

    #[test]
    fn reports_missing_paths() {
        let directory = tempfile::tempdir().expect("temp dir");
        let missing = directory.path().join("missing.txt");

        let error = inspect_path(&missing).expect_err("missing");
        assert_eq!(error.code, UiErrorCode::NotFound);
    }

    #[test]
    fn node_kind_is_exposed_for_the_screen() {
        // The DTO kind is used by callers to decide whether aggregates apply;
        // keep the mapping honest even though the module does not build rows.
        assert_ne!(NodeKind::File, NodeKind::Directory);
    }
}
