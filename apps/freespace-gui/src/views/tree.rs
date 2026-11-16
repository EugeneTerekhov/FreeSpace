//! Virtualized size tree with sorting, filtering and expansion.

use std::collections::HashSet;

use eframe::egui::{self, RichText};
use freespace_core::{
    query::{FilterSpec, SortDirection, SortKey, SortSpec, filtered_children, search},
    size::{Units, format_bytes},
    tree::{AccessState, FsTree, NodeId, NodeKind},
};
use freespace_scan::ScanResult;

/// Height of one tree row.
const ROW_HEIGHT: f32 = 20.0;

/// Search result cap: a filter should never freeze the UI.
const SEARCH_LIMIT: usize = 20_000;

/// State of the tree screen.
#[derive(Debug)]
pub struct TreeView {
    sort_key: SortKey,
    descending: bool,
    filter: String,
    owner_filter: Option<u32>,
    expanded: HashSet<NodeId>,
    selected: Option<NodeId>,
    initialized: bool,
}

impl Default for TreeView {
    fn default() -> Self {
        Self {
            sort_key: SortKey::Size,
            descending: true,
            filter: String::new(),
            owner_filter: None,
            expanded: HashSet::new(),
            selected: None,
            initialized: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Row {
    id: NodeId,
    level: usize,
    has_children: bool,
    expanded: bool,
}

impl TreeView {
    /// Clears expansion and filter state after a new scan.
    pub fn reset(&mut self) {
        self.expanded.clear();
        self.filter.clear();
        self.owner_filter = None;
        self.selected = None;
        self.initialized = false;
    }

    /// Restricts the tree to one owner index; `None` clears the filter.
    pub const fn set_owner_filter(&mut self, owner_id: Option<u32>) {
        self.owner_filter = owner_id;
    }

    /// Expands every visible directory so filtered files are not hidden.
    pub fn expand_all(&mut self, tree: &FsTree) {
        for id in 0..tree.len() as u32 {
            if tree.node(id).kind.is_directory() && !tree.node(id).is_removed() {
                self.expanded.insert(id);
            }
        }
        self.initialized = true;
    }

    /// Expands every ancestor of `id` and selects it.
    ///
    /// Used by the top-files and type screens to jump into the tree (FR-020).
    pub fn reveal(&mut self, tree: &FsTree, id: NodeId) {
        let mut current = tree.node(id).parent();
        while let Some(parent) = current {
            self.expanded.insert(parent);
            current = tree.node(parent).parent();
        }
        self.selected = Some(id);
        self.initialized = true;
    }

    /// Currently selected node.
    #[cfg(test)]
    #[must_use]
    pub const fn selected(&self) -> Option<NodeId> {
        self.selected
    }

    /// Draws the tree.
    pub fn ui(&mut self, ui: &mut egui::Ui, result: &ScanResult, units: Units) {
        let tree = &result.tree;
        let Some(root) = tree.roots().next() else {
            ui.label(t!("tree.empty").to_string());

            return;
        };
        if !self.initialized {
            self.expanded.insert(root);
            self.initialized = true;
        }

        self.toolbar(ui, tree);
        ui.separator();

        let sort = SortSpec::new(
            self.sort_key,
            if self.descending {
                SortDirection::Descending
            } else {
                SortDirection::Ascending
            },
        );
        let rows = if self.filter.trim().is_empty() {
            let filter = FilterSpec {
                include_hidden: true,
                owner_ids: self
                    .owner_filter
                    .map(|owner_id| vec![owner_id])
                    .unwrap_or_default(),
                ..FilterSpec::default()
            }
            .compile();
            match filter {
                Ok(filter) => self.expanded_rows(tree, root, &filter, sort),
                Err(_) => Vec::new(),
            }
        } else {
            self.search_rows(tree, root, sort)
        };

        if rows.is_empty() {
            ui.label(t!("tree.no_matches").to_string());

            return;
        }

        let available = ui.available_size_before_wrap();
        let mut toggled = None;
        let mut clicked = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .max_height(available.y.max(ROW_HEIGHT * 4.0))
            .show_rows(ui, ROW_HEIGHT, rows.len(), |ui, range| {
                for index in range {
                    let row = rows[index];
                    let (toggle, select) = self.row(ui, tree, row, units);
                    if toggle {
                        toggled = Some((row.id, row.expanded));
                    }
                    if select {
                        clicked = Some(row.id);
                    }
                }
            });

        if let Some(id) = clicked {
            self.selected = Some(id);
        }
        if let Some((id, was_expanded)) = toggled {
            if was_expanded {
                self.expanded.remove(&id);
            } else {
                self.expanded.insert(id);
            }
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, tree: &FsTree) {
        ui.horizontal_wrapped(|ui| {
            ui.label(t!("tree.sort_by").to_string());
            egui::ComboBox::from_id_salt("tree_sort")
                .selected_text(match self.sort_key {
                    SortKey::Name => t!("tree.sort.name").to_string(),
                    SortKey::Size => t!("tree.sort.size").to_string(),
                    SortKey::Modified => t!("tree.sort.modified").to_string(),
                    SortKey::Type => t!("tree.sort.type").to_string(),
                    SortKey::Owner => t!("tree.sort.owner").to_string(),
                    SortKey::Extension => t!("tree.sort.extension").to_string(),
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.sort_key,
                        SortKey::Name,
                        t!("tree.sort.name").to_string(),
                    );
                    ui.selectable_value(
                        &mut self.sort_key,
                        SortKey::Size,
                        t!("tree.sort.size").to_string(),
                    );
                    ui.selectable_value(
                        &mut self.sort_key,
                        SortKey::Modified,
                        t!("tree.sort.modified").to_string(),
                    );
                    ui.selectable_value(
                        &mut self.sort_key,
                        SortKey::Type,
                        t!("tree.sort.type").to_string(),
                    );
                });
            let arrow = if self.descending { "↓" } else { "↑" };
            if ui.button(arrow).clicked() {
                self.descending = !self.descending;
            }

            ui.separator();
            ui.label(t!("tree.filter").to_string());
            ui.add(
                egui::TextEdit::singleline(&mut self.filter)
                    .hint_text(t!("tree.filter_hint").to_string())
                    .desired_width(220.0),
            );
            if ui.button(t!("tree.expand_all").to_string()).clicked() {
                for id in 0..tree.len() as u32 {
                    if tree.node(id).kind.is_directory() && !tree.node(id).is_removed() {
                        self.expanded.insert(id);
                    }
                }
            }
            if ui.button(t!("tree.collapse_all").to_string()).clicked() {
                self.expanded.clear();
            }
            if let Some(owner) = self.owner_filter {
                ui.separator();
                if ui
                    .button(format!("{} #{owner} ✕", t!("nav.owners")))
                    .clicked()
                {
                    self.owner_filter = None;
                }
            }
        });
    }

    fn expanded_rows(
        &self,
        tree: &FsTree,
        root: NodeId,
        filter: &freespace_core::query::NodeFilter,
        sort: SortSpec,
    ) -> Vec<Row> {
        let mut rows = Vec::new();
        let mut stack = vec![(root, 0usize)];

        while let Some((id, level)) = stack.pop() {
            let mut children = filtered_children(tree, id, filter, sort);
            let has_children = !children.is_empty();
            let expanded = self.expanded.contains(&id);
            rows.push(Row {
                id,
                level,
                has_children,
                expanded,
            });
            if expanded && has_children {
                for child in children.drain(..).rev() {
                    stack.push((child, level + 1));
                }
            }
        }

        rows
    }

    fn search_rows(&self, tree: &FsTree, root: NodeId, sort: SortSpec) -> Vec<Row> {
        let spec = FilterSpec {
            pattern: Some(self.filter.trim().to_owned()),
            include_hidden: true,
            owner_ids: self
                .owner_filter
                .map(|owner_id| vec![owner_id])
                .unwrap_or_default(),
            ..FilterSpec::default()
        };
        let Ok(filter) = spec.compile() else {
            return Vec::new();
        };
        let matches = search(tree, root, &filter, sort, true, SEARCH_LIMIT);

        matches
            .into_iter()
            .map(|id| Row {
                id,
                level: tree.depth(id),
                has_children: tree.node(id).kind.is_directory(),
                expanded: true,
            })
            .collect()
    }

    fn row(&self, ui: &mut egui::Ui, tree: &FsTree, row: Row, units: Units) -> (bool, bool) {
        let node = tree.node(row.id);
        let mut toggle = false;
        let mut select = false;

        ui.horizontal(|ui| {
            ui.set_min_height(ROW_HEIGHT);
            ui.add_space(row.level as f32 * 14.0);
            if row.has_children {
                let symbol = if row.expanded { "⏷" } else { "⏵" };
                if ui.small_button(symbol).clicked() {
                    toggle = true;
                }
            } else {
                ui.add_space(18.0);
            }

            let is_directory = node.kind.is_directory();
            let mut text = RichText::new(tree.name(row.id));
            if is_directory {
                text = text.strong();
            }
            if node.access != AccessState::Ok {
                text = text.color(ui.visuals().warn_fg_color);
            }
            let label = ui.selectable_label(self.selected == Some(row.id), text);
            if label.clicked() {
                select = true;
                if row.has_children {
                    toggle = true;
                }
            }
            if label.double_clicked() && is_directory {
                toggle = true;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format_bytes(node.size.logical, units));
                if is_directory {
                    ui.label(format!(
                        "{} · {}",
                        t!("tree.files", count = node.file_count()),
                        t!("tree.dirs", count = node.dir_count())
                    ));
                }
                if node.access != AccessState::Ok {
                    ui.label(t!("tree.access_denied").to_string());
                }
                if node.kind == NodeKind::Hardlink {
                    ui.label(t!("tree.hardlink").to_string());
                }
            });
        });

        (toggle, select)
    }

    /// Full path of a row, used by tests.
    #[cfg(test)]
    #[must_use]
    pub fn path(tree: &FsTree, id: NodeId) -> String {
        freespace_platform::paths::display(&tree.path(id))
    }
}

#[cfg(test)]
mod tests {
    use freespace_core::{
        size::SizeMetrics,
        tree::{FsTree, NodeKind, NodeSpec},
    };

    use super::TreeView;

    fn fixture() -> (FsTree, u32, u32) {
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
                    name: "folder".to_owned(),
                    kind: NodeKind::Directory,
                    ..NodeSpec::default()
                },
            )
            .expect("folder");
        tree.add_node(
            Some(folder),
            NodeSpec {
                name: "deep.bin".to_owned(),
                kind: NodeKind::File,
                size: SizeMetrics::file(10),
                ..NodeSpec::default()
            },
        )
        .expect("deep");
        tree.recompute_totals();

        (tree, root, folder)
    }

    #[test]
    fn expands_and_collapses_rows() {
        let (tree, root, folder) = fixture();
        let mut view = TreeView::default();

        let filter = freespace_core::query::FilterSpec {
            include_hidden: true,
            ..Default::default()
        }
        .compile()
        .expect("filter");
        let sort = freespace_core::query::SortSpec::default();

        let collapsed = view.expanded_rows(&tree, root, &filter, sort);
        assert_eq!(collapsed.len(), 1);

        view.expanded.insert(root);
        let root_expanded = view.expanded_rows(&tree, root, &filter, sort);
        assert_eq!(root_expanded.len(), 2);
        assert_eq!(root_expanded[1].level, 1);

        view.expanded.insert(folder);
        let expanded = view.expanded_rows(&tree, root, &filter, sort);
        assert_eq!(expanded.len(), 3);
        assert_eq!(expanded[2].level, 2);

        view.reset();
        assert!(view.expanded.is_empty());
        assert!(view.filter.is_empty());
        assert!(!view.initialized);
        assert_eq!(TreeView::path(&tree, folder), r"C:\data\folder");
    }

    #[test]
    fn searches_with_ancestors() {
        let (tree, root, _folder) = fixture();
        let view = TreeView {
            filter: "deep.bin".to_owned(),
            ..TreeView::default()
        };
        let sort = freespace_core::query::SortSpec::default();

        let rows = view.search_rows(&tree, root, sort);

        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].level, 0);
        assert_eq!(rows[2].level, 2);
    }

    #[test]
    fn reveals_ancestors_and_selects_the_node() {
        let (tree, root, folder) = fixture();
        let deep = tree.children(folder).next().expect("deep file");
        let mut view = TreeView::default();

        view.reveal(&tree, deep);

        assert_eq!(view.selected(), Some(deep));
        assert!(view.expanded.contains(&root));
        assert!(view.expanded.contains(&folder));
        assert!(view.initialized);
    }
}
