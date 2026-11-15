//! Treemap navigation, hit testing and tooltip data.

use freespace_core::tree::{FsTree, NodeId};
use freespace_platform::paths;

use super::layout::TreemapItem;

/// Zoom state of a treemap: the node currently filling the view.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Navigation {
    current: Option<NodeId>,
}

impl Navigation {
    /// Starts at `root`.
    pub fn reset(&mut self, root: Option<NodeId>) {
        self.current = root;
    }

    /// Node currently shown.
    #[must_use]
    pub const fn current(&self) -> Option<NodeId> {
        self.current
    }

    /// Descends into a directory; returns `false` for files and links.
    pub fn enter(&mut self, tree: &FsTree, id: NodeId) -> bool {
        if !tree.node(id).kind.is_directory() {
            return false;
        }
        self.current = Some(id);
        true
    }

    /// Ascends to the parent; returns `false` at the root.
    pub fn up(&mut self, tree: &FsTree) -> bool {
        let Some(parent) = self.current.and_then(|id| tree.node(id).parent()) else {
            return false;
        };
        self.current = Some(parent);
        true
    }

    /// Jumps to an arbitrary node, for example from a breadcrumb.
    pub fn jump_to(&mut self, id: NodeId) {
        self.current = Some(id);
    }

    /// Breadcrumb path from the scan root to the current node.
    #[must_use]
    pub fn breadcrumbs(&self, tree: &FsTree) -> Vec<(NodeId, String)> {
        let Some(current) = self.current else {
            return Vec::new();
        };
        let mut trail = Vec::new();
        let mut node = Some(current);
        while let Some(id) = node {
            trail.push((id, tree.name(id).to_owned()));
            node = tree.node(id).parent();
        }
        trail.reverse();

        trail
    }
}

/// Information shown when the pointer hovers a rectangle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tooltip {
    /// File or directory name.
    pub name: String,
    /// Full path without the verbatim prefix.
    pub path: String,
    /// Logical size.
    pub size: u64,
    /// Share of the parent size in whole percents.
    pub share: u8,
    /// Number of files below the node.
    pub file_count: u32,
    /// Number of directories below the node.
    pub dir_count: u32,
    /// Whether the scanner could read the object.
    pub accessible: bool,
}

/// Builds tooltip data for `id` relative to `parent`.
#[must_use]
pub fn tooltip_for(tree: &FsTree, parent: Option<NodeId>, id: NodeId) -> Tooltip {
    let node = tree.node(id);
    let share = parent.map_or(0, |parent| {
        let total = tree.node(parent).size.logical;
        if total == 0 {
            0
        } else {
            ((node.size.logical.saturating_mul(100) / total).min(100)) as u8
        }
    });

    Tooltip {
        name: tree.name(id).to_owned(),
        path: paths::display(&tree.path(id)),
        size: node.size.logical,
        share,
        file_count: node.file_count(),
        dir_count: node.dir_count(),
        accessible: node.is_accessible(),
    }
}

/// Returns the node under the pointer.
#[must_use]
pub fn hit_test(items: &[TreemapItem], x: f32, y: f32) -> Option<NodeId> {
    items
        .iter()
        .rev()
        .find(|item| item.rect.contains(x, y))
        .map(|item| item.node)
}

#[cfg(test)]
mod tests {
    use freespace_core::{
        size::SizeMetrics,
        tree::{FsTree, NodeKind, NodeSpec},
    };

    use super::{Navigation, hit_test, tooltip_for};
    use crate::treemap::layout::{Rect, TreemapItem};

    fn fixture() -> (FsTree, u32, u32, u32) {
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
        let folder = tree
            .add_node(
                Some(root),
                NodeSpec {
                    name: "media".to_owned(),
                    kind: NodeKind::Directory,
                    ..NodeSpec::default()
                },
            )
            .expect("folder");
        let file = tree
            .add_node(
                Some(folder),
                NodeSpec {
                    name: "clip.mp4".to_owned(),
                    kind: NodeKind::File,
                    size: SizeMetrics::file(250),
                    ..NodeSpec::default()
                },
            )
            .expect("file");
        tree.add_node(
            Some(root),
            NodeSpec {
                name: "notes.txt".to_owned(),
                kind: NodeKind::File,
                size: SizeMetrics::file(50),
                ..NodeSpec::default()
            },
        )
        .expect("notes");
        tree.recompute_totals();

        (tree, root, folder, file)
    }

    #[test]
    fn navigates_down_and_up_with_breadcrumbs() {
        let (tree, root, folder, file) = fixture();
        let mut navigation = Navigation::default();
        navigation.reset(Some(root));

        assert!(navigation.enter(&tree, folder));
        assert!(!navigation.enter(&tree, file), "files cannot be entered");

        let crumbs = navigation.breadcrumbs(&tree);
        assert_eq!(crumbs.len(), 2);
        assert_eq!(crumbs[0].0, root);
        assert_eq!(crumbs[1].1, "media");

        assert!(navigation.up(&tree));
        assert_eq!(navigation.current(), Some(root));
        assert!(!navigation.up(&tree));

        navigation.jump_to(folder);
        assert_eq!(navigation.current(), Some(folder));
    }

    #[test]
    fn builds_tooltips_with_share_and_counts() {
        let (tree, root, folder, file) = fixture();

        let root_tip = tooltip_for(&tree, None, root);
        assert_eq!(root_tip.size, 300);
        assert_eq!(root_tip.file_count, 2);
        assert_eq!(root_tip.dir_count, 1);
        assert_eq!(root_tip.share, 0);

        let child_tip = tooltip_for(&tree, Some(root), folder);
        assert_eq!(child_tip.name, "media");
        assert_eq!(child_tip.share, 83);
        assert!(child_tip.path.contains("media"));

        let file_tip = tooltip_for(&tree, Some(folder), file);
        assert_eq!(file_tip.share, 100);
        assert!(file_tip.accessible);
    }

    #[test]
    fn hits_the_rectangle_under_the_pointer() {
        let items = [
            TreemapItem {
                node: 1,
                rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            },
            TreemapItem {
                node: 2,
                rect: Rect::new(10.0, 0.0, 10.0, 10.0),
            },
        ];

        assert_eq!(hit_test(&items, 5.0, 5.0), Some(1));
        assert_eq!(hit_test(&items, 15.0, 5.0), Some(2));
        assert_eq!(hit_test(&items, 50.0, 50.0), None);
    }
}
