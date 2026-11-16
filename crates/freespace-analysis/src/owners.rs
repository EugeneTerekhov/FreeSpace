//! Per-owner disk usage: SID interning, tree annotation and summaries
//! (FR-064, FR-065).
//!
//! Reading the owner of every file costs one security query per object, so the
//! work is explicit: [`annotate`] fills `FsNode::owner_id` and returns the
//! interned table, and [`summarize`] aggregates the result. Objects whose owner
//! cannot be resolved end up in the documented `unknown` category.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use freespace_core::{
    Result,
    tree::{FsTree, NodeId, NodeKind},
};
use freespace_platform::security::{self, OwnerIdentity};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

/// Display name of the category for objects without a resolvable owner.
pub const UNKNOWN_OWNER: &str = "unknown";

/// One interned owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerInfo {
    /// Dense index used by [`freespace_core::tree::FsNode::owner_id`].
    pub owner_id: u32,
    /// Security identifier.
    pub sid: String,
    /// Account name, or the SID when the name cannot be resolved.
    pub display_name: String,
}

/// Interned owner table.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OwnerTable {
    entries: Vec<OwnerInfo>,
    by_sid: HashMap<String, u32>,
}

impl OwnerTable {
    /// Creates an empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Interns an owner and returns its dense index.
    pub fn intern(&mut self, sid: &str, display_name: &str) -> u32 {
        if let Some(index) = self.by_sid.get(sid) {
            return *index;
        }

        let owner_id = u32::try_from(self.entries.len()).unwrap_or(u32::MAX - 1);
        let info = OwnerInfo {
            owner_id,
            sid: sid.to_owned(),
            display_name: display_name.to_owned(),
        };
        self.entries.push(info);
        self.by_sid.insert(sid.to_owned(), owner_id);

        owner_id
    }

    /// Number of interned owners.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` when no owner was interned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the interned owners.
    #[must_use]
    pub fn entries(&self) -> &[OwnerInfo] {
        &self.entries
    }

    /// Looks an owner up by index.
    #[must_use]
    pub fn get(&self, owner_id: u32) -> Option<&OwnerInfo> {
        self.entries.get(owner_id as usize)
    }

    /// Resolves an account name or a SID to its index.
    ///
    /// Matching is case-insensitive for names and exact for SIDs.
    #[must_use]
    pub fn find(&self, account: &str) -> Option<u32> {
        if let Some(index) = self.by_sid.get(account) {
            return Some(*index);
        }
        let account = account.to_lowercase();

        self.entries
            .iter()
            .find(|entry| entry.display_name.to_lowercase() == account)
            .map(|entry| entry.owner_id)
    }
}

/// One row of the owner report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerSummary {
    /// Dense owner index; [`u32::MAX`] for the unknown category.
    pub owner_id: u32,
    /// Security identifier, empty for the unknown category.
    pub sid: String,
    /// Account name or `unknown`.
    pub display_name: String,
    /// Total logical size of the owner's files.
    pub total_bytes: u64,
    /// Number of files owned by the account.
    pub file_count: u64,
}

impl OwnerSummary {
    /// Share of the analysed volume, in `0.0..=1.0`.
    #[must_use]
    pub fn share(&self, total: u64) -> f64 {
        if total == 0 {
            0.0
        } else {
            self.total_bytes as f64 / total as f64
        }
    }
}

/// Non-fatal problem encountered while reading ownership.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerError {
    /// Object that could not be inspected.
    pub path: PathBuf,
    /// Diagnostic message.
    pub message: String,
}

/// Result of [`annotate`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OwnerScan {
    /// Interned owners.
    pub table: OwnerTable,
    /// Aggregated report sorted by descending size.
    pub summaries: Vec<OwnerSummary>,
    /// Number of files whose owner could not be read.
    pub error_count: u64,
    /// Number of files placed in the unknown category.
    pub unknown_count: u64,
    /// Non-fatal owner-read failures.
    pub errors: Vec<OwnerError>,
}

impl OwnerScan {
    /// Total logical size covered by the report.
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.summaries
            .iter()
            .map(|summary| summary.total_bytes)
            .fold(0, u64::saturating_add)
    }
}

/// Reads ownership for every regular file and annotates the tree in place.
///
/// Links and directories are skipped: the report is about space occupied by
/// files, and a directory owner is not representative of its contents.
///
/// # Errors
/// Returns an error only when the tree cannot be enumerated; per-file failures
/// are collected in [`OwnerScan::errors`].
pub fn annotate(tree: &mut FsTree) -> Result<OwnerScan> {
    annotate_with(tree, security::owner_identity)
}

/// Same as [`annotate`] but with an injectable resolver.
///
/// The resolver runs on a Rayon pool, so it must be `Sync`.
///
/// # Errors
/// Returns an error only when the tree cannot be enumerated.
pub fn annotate_with<F>(tree: &mut FsTree, resolver: F) -> Result<OwnerScan>
where
    F: Fn(&Path) -> Result<Option<OwnerIdentity>> + Sync,
{
    let files = collect_files(tree);
    let resolved: Vec<std::result::Result<Option<OwnerIdentity>, freespace_core::Error>> =
        files.par_iter().map(|(_, path)| resolver(path)).collect();

    let mut table = OwnerTable::new();
    let mut errors = Vec::new();
    let mut error_count = 0u64;
    let mut unknown_count = 0u64;

    for ((node_id, path), result) in files.iter().zip(resolved) {
        match result {
            Ok(Some(identity)) => {
                let owner_id = table.intern(&identity.sid, &identity.display_name);
                tree.node_mut(*node_id).owner_id = Some(owner_id);
            }
            Ok(None) => {
                tree.node_mut(*node_id).owner_id = None;
                unknown_count = unknown_count.saturating_add(1);
            }
            Err(error) => {
                tree.node_mut(*node_id).owner_id = None;
                error_count = error_count.saturating_add(1);
                errors.push(OwnerError {
                    path: path.clone(),
                    message: error.to_string(),
                });
            }
        }
    }

    let summaries = summarize(tree, &table);

    Ok(OwnerScan {
        table,
        summaries,
        error_count,
        unknown_count,
        errors,
    })
}

/// Aggregates owned bytes per account.
#[must_use]
pub fn summarize(tree: &FsTree, table: &OwnerTable) -> Vec<OwnerSummary> {
    let mut totals: HashMap<Option<u32>, (u64, u64)> = HashMap::new();

    for index in 0..tree.len() {
        let Ok(node_id) = NodeId::try_from(index) else {
            continue;
        };
        let node = tree.node(node_id);
        if node.is_removed() || node.kind != NodeKind::File || !node.is_accessible() {
            continue;
        }
        let owner = node
            .owner_id
            .filter(|owner_id| table.get(*owner_id).is_some());
        let entry = totals.entry(owner).or_insert((0, 0));
        entry.0 = entry.0.saturating_add(node.size.logical);
        entry.1 = entry.1.saturating_add(1);
    }

    let mut summaries: Vec<OwnerSummary> = totals
        .into_iter()
        .map(|(owner_id, (total_bytes, file_count))| match owner_id {
            Some(owner_id) => {
                let info = table.get(owner_id);
                OwnerSummary {
                    owner_id,
                    sid: info.map(|info| info.sid.clone()).unwrap_or_default(),
                    display_name: info
                        .map(|info| info.display_name.clone())
                        .unwrap_or_else(|| UNKNOWN_OWNER.to_owned()),
                    total_bytes,
                    file_count,
                }
            }
            None => OwnerSummary {
                owner_id: u32::MAX,
                sid: String::new(),
                display_name: UNKNOWN_OWNER.to_owned(),
                total_bytes,
                file_count,
            },
        })
        .collect();

    summaries.sort_by(|left, right| {
        right
            .total_bytes
            .cmp(&left.total_bytes)
            .then_with(|| left.display_name.cmp(&right.display_name))
    });

    summaries
}

/// Returns the number of files that belong to `owner_id`.
#[must_use]
pub fn files_owned_by(tree: &FsTree, owner_id: u32) -> u64 {
    (0..tree.len())
        .filter_map(|index| NodeId::try_from(index).ok())
        .filter(|node_id| {
            let node = tree.node(*node_id);
            !node.is_removed() && node.kind == NodeKind::File && node.owner_id == Some(owner_id)
        })
        .count()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn collect_files(tree: &FsTree) -> Vec<(NodeId, PathBuf)> {
    let mut files = Vec::new();
    for index in 0..tree.len() {
        let Ok(node_id) = NodeId::try_from(index) else {
            continue;
        };
        let node = tree.node(node_id);
        if node.is_removed() || node.kind != NodeKind::File || !node.is_accessible() {
            continue;
        }
        files.push((node_id, tree.path(node_id)));
    }

    files
}

#[cfg(test)]
mod tests {
    use freespace_core::{
        size::SizeMetrics,
        tree::{FsTree, NodeKind, NodeSpec},
    };
    use freespace_platform::security::OwnerIdentity;

    use super::{OwnerTable, annotate_with, summarize};

    fn fixture() -> (FsTree, u32) {
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
        for (name, size) in [("alice.bin", 100u64), ("bob.bin", 250), ("shared.bin", 50)] {
            tree.add_node(
                Some(root),
                NodeSpec {
                    name: name.to_owned(),
                    kind: NodeKind::File,
                    size: SizeMetrics::file(size),
                    ..NodeSpec::default()
                },
            )
            .expect("file");
        }
        tree.recompute_totals();

        (tree, root)
    }

    fn identity(sid: &str, name: &str) -> Option<OwnerIdentity> {
        Some(OwnerIdentity {
            sid: sid.to_owned(),
            display_name: name.to_owned(),
        })
    }

    #[test]
    fn interns_sids_once_and_resolves_names() {
        let mut table = OwnerTable::new();
        let first = table.intern("S-1-5-21-1", r"PC\Alice");
        let second = table.intern("S-1-5-21-1", r"PC\Alice");
        let third = table.intern("S-1-5-21-2", r"PC\Bob");

        assert_eq!(first, second);
        assert_ne!(first, third);
        assert_eq!(table.len(), 2);
        assert_eq!(table.find(r"pc\alice"), Some(first));
        assert_eq!(table.find("S-1-5-21-2"), Some(third));
    }

    #[test]
    fn annotates_and_summarizes_files() {
        let (mut tree, root) = fixture();
        let scan = annotate_with(&mut tree, |path| {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            match name {
                "alice.bin" => Ok(identity("S-1-5-21-1", r"PC\Alice")),
                "bob.bin" => Ok(identity("S-1-5-21-2", r"PC\Bob")),
                _ => Ok(None),
            }
        })
        .expect("annotate");

        assert_eq!(scan.table.len(), 2);
        assert_eq!(scan.unknown_count, 1);
        assert_eq!(scan.summaries.len(), 3);
        assert_eq!(scan.summaries[0].display_name, r"PC\Bob");
        assert_eq!(scan.summaries[0].total_bytes, 250);
        assert_eq!(scan.summaries[0].file_count, 1);
        assert_eq!(scan.summaries[2].display_name, super::UNKNOWN_OWNER);
        assert_eq!(scan.total_bytes(), 400);
        assert_eq!(tree.node(root).owner_id, None);
    }

    #[test]
    fn keeps_unreadable_owners_unknown() {
        let (mut tree, _) = fixture();
        let scan = annotate_with(&mut tree, |_| {
            Err(freespace_core::Error::AccessDenied("denied".to_owned()))
        })
        .expect("annotate");

        assert_eq!(scan.error_count, 3);
        assert_eq!(scan.summaries.len(), 1);
        assert_eq!(scan.summaries[0].display_name, super::UNKNOWN_OWNER);
        assert_eq!(scan.summaries[0].total_bytes, 400);
        assert_eq!(summarize(&tree, &scan.table), scan.summaries);
    }
}
