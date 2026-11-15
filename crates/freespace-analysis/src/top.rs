//! Flat top-N lists of the largest files.
//!
//! A full sort of a multi-million-node tree is wasteful when the user only
//! wants the hundred or thousand heaviest files, so the selection keeps a
//! bounded min-heap: memory stays `O(N)` and the whole tree is visited exactly
//! once (FR-019, SC-005).

use std::{cmp::Reverse, collections::BinaryHeap, path::PathBuf};

use freespace_core::{
    category::{FileCategory, category_for},
    size::SizeMetrics,
    time::Timestamp,
    tree::{AccessState, FsTree, NodeId, NodeKind},
};

/// One entry of the top-files list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopFile {
    /// Tree node.
    pub node_id: NodeId,
    /// Full path of the file.
    pub path: PathBuf,
    /// File name without directories.
    pub name: String,
    /// Size along every dimension.
    pub size: SizeMetrics,
    /// Last modification time.
    pub modified: Timestamp,
    /// Coarse category derived from the extension.
    pub category: FileCategory,
    /// Whether the scanner could read the file.
    pub access: AccessState,
}

impl TopFile {
    /// Returns `true` when the file is accessible.
    #[must_use]
    pub const fn is_accessible(&self) -> bool {
        matches!(self.access, AccessState::Ok)
    }
}

/// Returns the `limit` largest files in the tree, heaviest first.
///
/// Directories and links are skipped; the order is deterministic: equal sizes
/// are broken by the full path so repeated runs report the same list.
#[must_use]
pub fn top_files(tree: &FsTree, limit: usize) -> Vec<TopFile> {
    if limit == 0 {
        return Vec::new();
    }

    let mut heap: BinaryHeap<Reverse<Candidate>> = BinaryHeap::with_capacity(limit);
    for node_id in 0..tree.len() as u32 {
        if tree.node(node_id).is_removed() {
            continue;
        }
        let node = tree.node(node_id);
        if node.kind.is_directory() || matches!(node.kind, NodeKind::Symlink | NodeKind::Junction) {
            continue;
        }

        let candidate = Candidate {
            logical: node.size.logical,
            node_id,
        };
        if heap.len() < limit {
            heap.push(Reverse(candidate));
        } else if heap
            .peek()
            .is_some_and(|Reverse(smallest)| candidate > *smallest)
        {
            heap.pop();
            heap.push(Reverse(candidate));
        }
    }

    let mut files: Vec<TopFile> = heap
        .into_iter()
        .map(|Reverse(candidate)| {
            let id = candidate.node_id;
            let node = tree.node(id);
            let name = tree.name(id).to_owned();

            TopFile {
                node_id: id,
                path: tree.path(id),
                category: category_for(&name),
                name,
                size: node.size,
                modified: node.timestamps.modified,
                access: node.access,
            }
        })
        .collect();

    files.sort_by(|left, right| {
        right
            .size
            .logical
            .cmp(&left.size.logical)
            .then_with(|| left.path.cmp(&right.path))
    });

    files
}

/// Heap entry ordered naturally by size; the heap wraps it in [`Reverse`] so
/// that the smallest candidate stays at the top.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Candidate {
    logical: u64,
    node_id: NodeId,
}

#[cfg(test)]
mod tests {
    use freespace_core::{
        size::SizeMetrics,
        tree::{FsTree, NodeKind, NodeSpec},
    };

    use super::top_files;

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
        for (name, size) in [
            ("small.bin", 10),
            ("large.bin", 400),
            ("medium.bin", 100),
            ("also-large.bin", 400),
        ] {
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

        tree
    }

    #[test]
    fn returns_the_largest_files_first() {
        let tree = tree();
        let files = top_files(&tree, 3);

        assert_eq!(files.len(), 3);
        assert_eq!(files[0].size.logical, 400);
        assert_eq!(files[1].size.logical, 400);
        assert_eq!(files[2].size.logical, 100);
        assert!(files[0].path <= files[1].path, "ties are path-ordered");
    }

    #[test]
    fn handles_limits_and_nulls() {
        let tree = tree();
        assert!(top_files(&tree, 0).is_empty());
        assert_eq!(top_files(&tree, 100).len(), 4);
    }

    #[test]
    fn skips_directories() {
        let tree = tree();
        let files = top_files(&tree, 100);

        assert!(
            files
                .iter()
                .all(|file| !file.name.eq_ignore_ascii_case("data")),
            "the scan root must not appear in the top list"
        );
    }
}
