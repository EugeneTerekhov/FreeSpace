//! Exact space accounting with hard-link de-duplication and ADS (FR-047…FR-050).
//!
//! A logical byte count is not the same as the physical contribution: hard links
//! share one set of clusters, compression and sparseness reduce the allocated
//! size, and alternate data streams are not part of the logical length at all.
//! This module keeps all dimensions separate and never inflates totals.

use std::collections::HashMap;

use freespace_core::{
    Result,
    size::{SizeMethod, SizeMetrics},
    tree::{AdsStream, FsTree, NodeId, NodeKind},
};
use freespace_platform::ads;

/// Which size dimension a report should display.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SizeMode {
    /// Sum of file lengths, every hard link counted.
    #[default]
    Logical,
    /// Space allocated on the volume.
    OnDisk,
    /// Physical contribution with hard links counted once.
    Physical,
}

impl SizeMode {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Logical => "logical",
            Self::OnDisk => "on-disk",
            Self::Physical => "physical",
        }
    }

    /// Parses a command-line value.
    #[must_use]
    pub fn parse_value(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "logical" => Some(Self::Logical),
            "on-disk" | "ondisk" | "allocated" => Some(Self::OnDisk),
            "physical" => Some(Self::Physical),
            _ => None,
        }
    }

    /// Extracts the selected dimension.
    #[must_use]
    pub const fn value(self, metrics: SizeMetrics) -> u64 {
        match self {
            Self::Logical => metrics.logical,
            Self::OnDisk => metrics.total_on_disk(),
            Self::Physical => metrics.physical.saturating_add(metrics.ads),
        }
    }
}

/// Aggregate space accounting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Accounting {
    /// Sum of file lengths, every hard link counted.
    pub logical: u64,
    /// Allocated bytes, compression and sparse aware.
    pub on_disk: u64,
    /// Physical bytes, hard links counted once.
    pub physical: u64,
    /// Alternate-stream bytes.
    pub ads: u64,
    /// Bytes shared by additional hard links.
    pub shared: u64,
    /// Number of regular files and hard links.
    pub file_count: u64,
    /// Number of additional hard links that were not counted physically.
    pub link_count: u64,
    /// Whether the values are exact.
    pub method: SizeMethod,
}

impl Accounting {
    /// Savings from compression and sparseness.
    #[must_use]
    pub const fn savings(&self) -> u64 {
        self.logical.saturating_sub(self.on_disk)
    }

    /// Returns `true` when the accounting is only an estimate.
    #[must_use]
    pub const fn is_estimated(&self) -> bool {
        matches!(self.method, SizeMethod::Estimated)
    }
}

/// Computes accounting for the live tree without changing it.
#[must_use]
pub fn summarize(tree: &FsTree) -> Accounting {
    let mut accounting = Accounting {
        logical: 0,
        on_disk: 0,
        physical: 0,
        ads: 0,
        shared: 0,
        file_count: 0,
        link_count: 0,
        method: SizeMethod::Exact,
    };
    let mut seen: HashMap<u64, u64> = HashMap::new();

    for id in live_nodes(tree) {
        let node = tree.node(id);
        if !matches!(node.kind, NodeKind::File | NodeKind::Hardlink) {
            continue;
        }
        accounting.file_count = accounting.file_count.saturating_add(1);
        accounting.logical = accounting.logical.saturating_add(node.size.logical);
        accounting.on_disk = accounting.on_disk.saturating_add(node.size.on_disk);
        accounting.ads = accounting.ads.saturating_add(node.size.ads);
        if node.size.is_estimated() || !node.is_accessible() {
            accounting.method = SizeMethod::Estimated;
        }

        if node.file_ref != 0 {
            if let Some(previous) = seen.get(&node.file_ref) {
                accounting.link_count = accounting.link_count.saturating_add(1);
                accounting.shared = accounting.shared.saturating_add(*previous);
                continue;
            }
            seen.insert(node.file_ref, node.size.physical);
        }
        accounting.physical = accounting.physical.saturating_add(node.size.physical);
    }

    accounting
}

/// Rewrites `physical` so every hard link is counted exactly once.
///
/// Returns the number of additional links whose physical size was zeroed.
pub fn dedupe_physical(tree: &mut FsTree) -> u64 {
    let mut seen: HashMap<u64, ()> = HashMap::new();
    let mut duplicates = 0u64;
    let ids = live_nodes(tree);

    for id in ids {
        let node = tree.node_mut(id);
        if !matches!(node.kind, NodeKind::File | NodeKind::Hardlink) || node.file_ref == 0 {
            continue;
        }
        if seen.insert(node.file_ref, ()).is_some() {
            node.size.physical = 0;
            duplicates = duplicates.saturating_add(1);
        }
    }
    if duplicates > 0 {
        tree.recompute_totals();
    }

    duplicates
}

/// Result of ADS enrichment.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AdsReport {
    /// Files that were inspected.
    pub files: usize,
    /// Files that carry at least one named stream.
    pub files_with_streams: usize,
    /// Streams discovered.
    pub streams: usize,
    /// Files that could not be read.
    pub errors: usize,
}

/// Enumerates alternate data streams for every regular file.
///
/// Errors are counted and skipped: a single protected file must not fail the
/// whole analysis.
pub fn enrich_ads(tree: &mut FsTree) -> AdsReport {
    let mut report = AdsReport::default();
    for id in live_nodes(tree) {
        if !matches!(tree.node(id).kind, NodeKind::File | NodeKind::Hardlink) {
            continue;
        }
        report.files = report.files.saturating_add(1);
        let path = tree.path(id);
        match ads::list_streams(&path) {
            Ok(streams) => {
                if streams.is_empty() {
                    continue;
                }
                report.files_with_streams = report.files_with_streams.saturating_add(1);
                report.streams = report.streams.saturating_add(streams.len());
                let total: u64 = streams.iter().map(|stream| stream.size).sum();
                let node = tree.node_mut(id);
                node.ads = Some(
                    streams
                        .into_iter()
                        .map(|stream| AdsStream {
                            name: stream.name,
                            size: stream.size,
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                );
                node.size.ads = total;
            }
            Err(_) => report.errors = report.errors.saturating_add(1),
        }
    }

    if report.files_with_streams > 0 {
        tree.recompute_totals();
    }

    report
}

/// Refreshes the allocated size of every file with `GetCompressedFileSizeW`.
///
/// This is an expensive per-file call and is therefore opt-in.
///
/// # Errors
/// Returns an error when the tree cannot be updated.
pub fn refresh_allocated(tree: &mut FsTree) -> Result<usize> {
    let mut updated = 0usize;
    for id in live_nodes(tree) {
        if !matches!(tree.node(id).kind, NodeKind::File | NodeKind::Hardlink) {
            continue;
        }
        let path = tree.path(id);
        if let Ok(allocated) = freespace_platform::allocated::allocated_size(&path) {
            let node = tree.node_mut(id);
            node.size.on_disk = allocated;
            updated += 1;
        }
    }
    if updated > 0 {
        tree.recompute_totals();
    }

    Ok(updated)
}

fn live_nodes(tree: &FsTree) -> Vec<NodeId> {
    let mut ids = Vec::new();
    let mut stack: Vec<NodeId> = tree.roots().collect();

    while let Some(id) = stack.pop() {
        if tree.node(id).is_removed() {
            continue;
        }
        ids.push(id);
        stack.extend(tree.children(id));
    }

    ids
}

#[cfg(test)]
mod tests {
    use freespace_core::{
        size::SizeMetrics,
        tree::{FsTree, NodeKind, NodeSpec},
    };

    use super::{SizeMode, dedupe_physical, summarize};

    #[test]
    fn counts_hard_links_once() {
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
        for name in ["a.bin", "b.bin"] {
            tree.add_node(
                Some(root),
                NodeSpec {
                    name: name.to_owned(),
                    kind: NodeKind::Hardlink,
                    file_ref: 42,
                    size: SizeMetrics::exact(1_000, 1_000, 1_000, 0),
                    ..NodeSpec::default()
                },
            )
            .expect("link");
        }
        tree.recompute_totals();

        let accounting = summarize(&tree);
        assert_eq!(accounting.logical, 2_000);
        assert_eq!(accounting.physical, 1_000);
        assert_eq!(accounting.shared, 1_000);
        assert_eq!(accounting.link_count, 1);

        assert_eq!(dedupe_physical(&mut tree), 1);
        assert_eq!(summarize(&tree).physical, 1_000);
        assert_eq!(tree.node(root).size.physical, 1_000);
    }

    #[test]
    fn exposes_all_size_modes() {
        let metrics = SizeMetrics::exact(1_000, 600, 800, 50);

        assert_eq!(SizeMode::Logical.value(metrics), 1_000);
        assert_eq!(SizeMode::OnDisk.value(metrics), 650);
        assert_eq!(SizeMode::Physical.value(metrics), 850);
        assert_eq!(SizeMode::parse_value("physical"), Some(SizeMode::Physical));
        assert_eq!(SizeMode::parse_value("magic"), None);
    }
}
