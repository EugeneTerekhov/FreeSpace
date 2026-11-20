//! Aggregation of occupied space by extension and by coarse category.
//!
//! The report answers "what kind of data fills the disk": every row carries the
//! number of objects, the summed size and its share of the analysed file bytes.
//! Extension groups contain only regular files and hard links; the category
//! report additionally counts directories (except volume roots) in the
//! `folder` category. Directory sizes are aggregates of their children, so a
//! folder row counts objects but always contributes zero file bytes (FR-019).

use std::collections::{HashMap, HashSet};

pub use freespace_core::category::{FileCategory, category_for, category_for_extension};
use freespace_core::{
    query::extension_of,
    size::SizeMetrics,
    tree::{FsTree, NodeId, NodeKind},
};

/// One extension row of the type report.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeGroup {
    /// Lower-cased extension without the dot; empty when the file has none.
    pub extension: String,
    /// Coarse category of the extension.
    pub category: FileCategory,
    /// Number of files with this extension.
    pub count: u64,
    /// Summed size of those files.
    pub size: SizeMetrics,
    /// Share of the analysed file bytes in `0.0..=1.0`.
    pub share: f64,
}

/// One category row of the type report.
#[derive(Debug, Clone, PartialEq)]
pub struct CategoryTotal {
    /// Category.
    pub category: FileCategory,
    /// Number of files in the category.
    pub count: u64,
    /// Summed size of the category.
    pub size: SizeMetrics,
    /// Share of the analysed file bytes in `0.0..=1.0`.
    pub share: f64,
}

/// Returns `true` for nodes that contribute file bytes.
#[must_use]
pub fn is_regular_file(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::File | NodeKind::Hardlink)
}

/// Aggregates occupied space by extension, heaviest extension first.
#[must_use]
pub fn group_by_extension(tree: &FsTree) -> Vec<TypeGroup> {
    let mut groups: HashMap<String, (u64, SizeMetrics)> = HashMap::new();
    let mut total = 0u64;

    for node_id in file_nodes(tree) {
        let node = tree.node(node_id);
        let extension = extension_of(tree.name(node_id)).to_ascii_lowercase();
        let entry = groups.entry(extension).or_insert((0, SizeMetrics::ZERO));
        entry.0 = entry.0.saturating_add(1);
        entry.1 += node.size;
        total = total.saturating_add(node.size.logical);
    }

    let mut rows: Vec<TypeGroup> = groups
        .into_iter()
        .map(|(extension, (count, size))| TypeGroup {
            category: category_for_extension(&extension),
            extension,
            count,
            size,
            share: share_of(size.logical, total),
        })
        .collect();

    rows.sort_by(|left, right| {
        right
            .size
            .logical
            .cmp(&left.size.logical)
            .then_with(|| left.extension.cmp(&right.extension))
    });

    rows
}

/// Aggregates occupied space by coarse category, largest category first.
///
/// Regular files and hard links are grouped by extension-derived category.
/// Directories below the roots are counted in [`FileCategory::Folder`]; volume
/// roots are not counted. A directory's size is the sum of its children, so
/// adding it would double count file bytes — folder rows carry a count but no
/// size.
#[must_use]
pub fn group_by_category(tree: &FsTree) -> Vec<CategoryTotal> {
    let mut totals: HashMap<FileCategory, (u64, SizeMetrics)> = HashMap::new();
    let mut total = 0u64;
    let roots: HashSet<NodeId> = tree.roots().collect();

    let mut stack: Vec<NodeId> = tree.roots().collect();
    while let Some(node_id) = stack.pop() {
        if tree.node(node_id).is_removed() {
            continue;
        }
        let node = tree.node(node_id);
        if roots.contains(&node_id) {
            // Volume roots frame the report; they are not counted objects.
        } else if node.kind.is_directory() {
            let entry = totals
                .entry(FileCategory::Folder)
                .or_insert((0, SizeMetrics::ZERO));
            entry.0 = entry.0.saturating_add(1);
        } else if is_regular_file(node.kind) {
            let category = category_for(tree.name(node_id));
            let entry = totals.entry(category).or_insert((0, SizeMetrics::ZERO));
            entry.0 = entry.0.saturating_add(1);
            entry.1 += node.size;
            total = total.saturating_add(node.size.logical);
        }
        stack.extend(tree.children(node_id));
    }

    let mut rows: Vec<CategoryTotal> = totals
        .into_iter()
        .map(|(category, (count, size))| CategoryTotal {
            category,
            count,
            size,
            share: share_of(size.logical, total),
        })
        .collect();

    rows.sort_by(|left, right| {
        right
            .size
            .logical
            .cmp(&left.size.logical)
            .then_with(|| left.category.as_str().cmp(right.category.as_str()))
    });

    rows
}

/// Ids of every regular file below the roots, skipping tombstones.
fn file_nodes(tree: &FsTree) -> Vec<NodeId> {
    let mut ids = Vec::new();
    let mut stack: Vec<NodeId> = tree.roots().collect();

    while let Some(id) = stack.pop() {
        if tree.node(id).is_removed() {
            continue;
        }
        if is_regular_file(tree.node(id).kind) {
            ids.push(id);
        }
        stack.extend(tree.children(id));
    }

    ids
}

fn share_of(part: u64, total: u64) -> f64 {
    if total == 0 {
        return 0.0;
    }

    part as f64 / total as f64
}

#[cfg(test)]
mod tests {
    use freespace_core::{
        size::SizeMetrics,
        tree::{FsTree, NodeKind, NodeSpec},
    };

    use super::{FileCategory, group_by_category, group_by_extension};

    fn tree() -> FsTree {
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
        for (parent, name, size) in [
            (root, "one.txt", 100),
            (nested, "two.TXT", 300),
            (root, "movie.mkv", 600),
            (root, "README", 50),
        ] {
            tree.add_node(
                Some(parent),
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

        tree
    }

    #[test]
    fn groups_extensions_case_insensitively() {
        let rows = group_by_extension(&tree());
        let txt = rows
            .iter()
            .find(|row| row.extension == "txt")
            .expect("txt row");

        assert_eq!(txt.count, 2);
        assert_eq!(txt.size.logical, 400);
        assert_eq!(txt.category, FileCategory::Document);
        assert!((txt.share - 400.0 / 1_050.0).abs() < 1e-9);
        assert_eq!(rows[0].extension, "mkv", "heaviest extension first");
        assert_eq!(
            rows.iter()
                .find(|row| row.extension.is_empty())
                .map(|row| row.count),
            Some(1)
        );
    }

    #[test]
    fn groups_categories() {
        let rows = group_by_category(&tree());
        let video = rows
            .iter()
            .find(|row| row.category == FileCategory::Video)
            .expect("video row");
        let documents = rows
            .iter()
            .find(|row| row.category == FileCategory::Document)
            .expect("document row");

        assert_eq!(video.count, 1);
        assert_eq!(video.size.logical, 600);
        assert_eq!(documents.count, 2);
        assert_eq!(documents.size.logical, 400);

        // Only the nested directory is counted; the root is a volume, not a
        // folder in the report. A folder row never adds file bytes again.
        let folders = rows
            .iter()
            .find(|row| row.category == FileCategory::Folder)
            .expect("folder row");
        assert_eq!(folders.count, 1);
        assert_eq!(folders.size, SizeMetrics::ZERO);
        assert_eq!(folders.share, 0.0);
        assert_eq!(rows.iter().map(|row| row.count).sum::<u64>(), 5);
        assert_eq!(rows.iter().map(|row| row.size.logical).sum::<u64>(), 1_050);
    }

    #[test]
    fn empty_tree_produces_no_rows() {
        assert!(group_by_extension(&FsTree::new()).is_empty());
        assert!(group_by_category(&FsTree::new()).is_empty());
    }
}
