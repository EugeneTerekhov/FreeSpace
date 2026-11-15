//! Largest files and file-type report screens.
//!
//! Both views are computed from the completed scan on first use and are
//! invalidated by [`TopTypesState::reset`] after a new scan. Double-clicking a
//! top-file row returns its node id so the shell can reveal it in the tree
//! (FR-020).

use eframe::egui;
use freespace_analysis::{
    CategoryTotal, TopFile, TypeGroup, group_by_category, group_by_extension, top_files,
};
use freespace_core::{
    size::{Units, format_bytes},
    tree::NodeId,
};
use freespace_platform::paths;
use freespace_scan::ScanResult;

use crate::treemap::colors::CategoryColor;

/// Available top-file limits.
const LIMITS: [usize; 2] = [100, 1_000];

/// Which tab is visible.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TopTypesTab {
    /// Flat list of the largest files.
    #[default]
    Top,
    /// Space grouped by extension and category.
    Types,
}

/// State of the top/types screens.
#[derive(Debug)]
pub struct TopTypesState {
    tab: TopTypesTab,
    limit: usize,
    top: Vec<TopFile>,
    types: Vec<TypeGroup>,
    categories: Vec<CategoryTotal>,
    computed_limit: usize,
    computed: bool,
}

impl Default for TopTypesState {
    fn default() -> Self {
        Self {
            tab: TopTypesTab::default(),
            limit: LIMITS[0],
            top: Vec::new(),
            types: Vec::new(),
            categories: Vec::new(),
            computed_limit: 0,
            computed: false,
        }
    }
}

impl TopTypesState {
    /// Current tab.
    #[cfg(test)]
    #[must_use]
    pub const fn tab(&self) -> TopTypesTab {
        self.tab
    }

    /// Switches the tab.
    pub const fn set_tab(&mut self, tab: TopTypesTab) {
        self.tab = tab;
    }

    /// Current top-file limit.
    #[cfg(test)]
    #[must_use]
    pub const fn limit(&self) -> usize {
        self.limit
    }

    /// Invalidates cached results after a new scan.
    pub fn reset(&mut self) {
        self.top.clear();
        self.types.clear();
        self.categories.clear();
        self.computed = false;
        self.computed_limit = 0;
    }

    /// Draws the screen; returns a node id when the user asks to reveal one.
    pub fn ui(&mut self, ui: &mut egui::Ui, result: &ScanResult, units: Units) -> Option<NodeId> {
        self.ensure(result);

        let mut selected = None;
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.tab, TopTypesTab::Top, t!("nav.top").to_string());
            ui.selectable_value(
                &mut self.tab,
                TopTypesTab::Types,
                t!("nav.types").to_string(),
            );
            ui.separator();
            if self.tab == TopTypesTab::Top {
                ui.label(t!("top.limit").to_string());
                egui::ComboBox::from_id_salt("top_limit")
                    .selected_text(self.limit.to_string())
                    .show_ui(ui, |ui| {
                        for limit in LIMITS {
                            ui.selectable_value(&mut self.limit, limit, limit.to_string());
                        }
                    });
            }
        });
        ui.separator();

        match self.tab {
            TopTypesTab::Top => selected = self.top_tab(ui, units),
            TopTypesTab::Types => self.types_tab(ui, units),
        }

        selected
    }

    fn ensure(&mut self, result: &ScanResult) {
        if self.computed && self.computed_limit == self.limit && self.tab == TopTypesTab::Top {
            return;
        }
        if !self.computed || self.computed_limit != self.limit {
            self.top = top_files(&result.tree, self.limit);
            self.computed_limit = self.limit;
        }
        if !self.computed {
            self.types = group_by_extension(&result.tree);
            self.categories = group_by_category(&result.tree);
            self.computed = true;
        }
    }

    fn top_tab(&mut self, ui: &mut egui::Ui, units: Units) -> Option<NodeId> {
        if self.top.is_empty() {
            ui.label(t!("top.empty").to_string());

            return None;
        }

        let mut selected = None;
        egui::Grid::new("top_files")
            .num_columns(4)
            .striped(true)
            .show(ui, |ui| {
                ui.strong("#");
                ui.strong(t!("top.path").to_string());
                ui.strong(t!("top.size").to_string());
                ui.strong(t!("top.modified").to_string());
                ui.end_row();

                for (rank, file) in self.top.iter().enumerate() {
                    ui.label(format!("{}", rank + 1));
                    let mut text = egui::RichText::new(paths::display(&file.path));
                    if !file.is_accessible() {
                        text = text.color(ui.visuals().warn_fg_color);
                    }
                    let response = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                    if response.double_clicked() {
                        selected = Some(file.node_id);
                    }
                    ui.label(format_bytes(file.size.logical, units));
                    ui.label(file.modified.format_utc());
                    ui.end_row();
                }
            });

        selected
    }

    fn types_tab(&mut self, ui: &mut egui::Ui, units: Units) {
        if self.types.is_empty() {
            ui.label(t!("types.empty").to_string());

            return;
        }

        ui.label(t!("types.by_category").to_string());
        egui::Grid::new("type_categories")
            .num_columns(4)
            .striped(true)
            .show(ui, |ui| {
                for category in &self.categories {
                    ui.colored_label(
                        category.category.color(),
                        t!(category.category.label_key()).to_string(),
                    );
                    ui.label(format!("{}", category.count));
                    ui.label(format_bytes(category.size.logical, units));
                    ui.label(format!("{:.1}%", category.share * 100.0));
                    ui.end_row();
                }
            });

        ui.add_space(12.0);
        ui.label(t!("types.by_extension").to_string());
        egui::Grid::new("type_extensions")
            .num_columns(4)
            .striped(true)
            .show(ui, |ui| {
                ui.strong(t!("types.extension").to_string());
                ui.strong(t!("types.count").to_string());
                ui.strong(t!("types.size").to_string());
                ui.strong(t!("types.share").to_string());
                ui.end_row();

                for group in &self.types {
                    ui.colored_label(
                        group.category.color(),
                        if group.extension.is_empty() {
                            t!("types.no_extension").to_string()
                        } else {
                            format!(".{}", group.extension)
                        },
                    );
                    ui.label(format!("{}", group.count));
                    ui.label(format_bytes(group.size.logical, units));
                    ui.label(format!("{:.1}%", group.share * 100.0));
                    ui.end_row();
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::{LIMITS, TopTypesState, TopTypesTab};

    #[test]
    fn defaults_to_the_top_tab_and_the_smallest_limit() {
        let state = TopTypesState::default();

        assert_eq!(state.tab(), TopTypesTab::Top);
        assert_eq!(state.limit(), 100);
        assert_eq!(LIMITS, [100, 1_000]);
    }

    #[test]
    fn reset_invalidates_cached_results() {
        let mut state = TopTypesState::default();
        state.set_tab(TopTypesTab::Types);
        state.reset();

        assert_eq!(state.tab(), TopTypesTab::Types);
        assert!(state.top.is_empty());
        assert!(state.types.is_empty());
        assert!(!state.computed);
    }
}
