//! Empty-directory discovery over an already built [`FsTree`] (FR-036).
//!
//! A directory is empty when its whole subtree contains no files and no link
//! targets. Nested empty directories are reported individually so that a
//! cleanup screen can offer the deepest ones first; deleting them from the
//! deepest level removes the whole branch without extra passes.

use std::path::PathBuf;

use freespace_core::tree::{FsTree, NodeId, NodeKind};

/// One empty directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmptyDir {
    /// Node inside the analysed tree.
    pub node_id: NodeId,
    /// Full path.
    pub path: PathBuf,
    /// Depth from the scan root; roots have depth zero.
    pub depth: usize,
}

/// Finds empty directories, deepest first.
///
/// Scan roots are not reported by default because removing an analysed root is
/// almost never what the user means.
#[must_use]
pub fn find_empty_dirs(tree: &FsTree, include_roots: bool) -> Vec<EmptyDir> {
    let mut empty = Vec::new();

    for root in tree.roots() {
        collect(tree, root, include_roots, &mut empty);
    }

    empty.sort_by(|left, right| {
        right
            .depth
            .cmp(&left.depth)
            .then_with(|| right.path.cmp(&left.path))
    });

    empty
}

/// Returns `true` when the subtree of `id` contains no data.
fn collect(tree: &FsTree, id: NodeId, include_root: bool, empty: &mut Vec<EmptyDir>) -> bool {
    let node = tree.node(id);
    if node.is_removed() {
        return true;
    }
    if !node.kind.is_directory() {
        return false;
    }
    // Links are not traversed and are not empty directories.
    if node.kind != NodeKind::Directory {
        return false;
    }

    let mut is_empty = true;
    for child in tree.children(id) {
        if !collect(tree, child, true, empty) {
            is_empty = false;
        }
    }

    if is_empty && (include_root || node.parent().is_some()) {
        empty.push(EmptyDir {
            node_id: id,
            path: tree.path(id),
            depth: tree.depth(id),
        });
    }

    is_empty
}

#[cfg(test)]
mod tests {
    use freespace_core::{
        size::SizeMetrics,
        tree::{FsTree, NodeKind, NodeSpec},
    };

    use super::find_empty_dirs;

    fn directory(name: &str) -> NodeSpec {
        NodeSpec {
            name: name.to_owned(),
            kind: NodeKind::Directory,
            ..NodeSpec::default()
        }
    }

    fn file(name: &str, size: u64) -> NodeSpec {
        NodeSpec {
            name: name.to_owned(),
            kind: NodeKind::File,
            size: SizeMetrics::file(size),
            ..NodeSpec::default()
        }
    }

    #[test]
    fn finds_nested_empty_directories_deepest_first() {
        let mut tree = FsTree::new();
        let root = tree.add_node(None, directory("root")).expect("root");
        let empty = tree
            .add_node(Some(root), directory("empty"))
            .expect("empty");
        let nested = tree
            .add_node(Some(empty), directory("nested"))
            .expect("nested");
        let populated = tree
            .add_node(Some(root), directory("populated"))
            .expect("populated");
        tree.add_node(Some(populated), file("payload.bin", 10))
            .expect("file");
        tree.recompute_totals();

        let found = find_empty_dirs(&tree, false);
        let paths: Vec<_> = found.iter().map(|entry| entry.path.clone()).collect();

        assert_eq!(found.len(), 2);
        assert_eq!(found[0].node_id, nested);
        assert_eq!(found[1].node_id, empty);
        assert!(paths.iter().all(|path| !path.ends_with("populated")));
        assert!(paths.iter().all(|path| path.file_name().is_some()));
    }

    #[test]
    fn skips_roots_unless_requested() {
        let mut tree = FsTree::new();
        tree.add_node(None, directory("root")).expect("root");

        assert!(find_empty_dirs(&tree, false).is_empty());
        assert_eq!(find_empty_dirs(&tree, true).len(), 1);
    }

    #[test]
    fn treats_links_as_data() {
        let mut tree = FsTree::new();
        let root = tree.add_node(None, directory("root")).expect("root");
        let linked = tree
            .add_node(Some(root), directory("folder"))
            .expect("folder");
        tree.add_node(
            Some(linked),
            NodeSpec {
                name: "junction".to_owned(),
                kind: NodeKind::Junction,
                ..NodeSpec::default()
            },
        )
        .expect("junction");
        tree.recompute_totals();

        assert!(find_empty_dirs(&tree, false).is_empty());
    }
}
