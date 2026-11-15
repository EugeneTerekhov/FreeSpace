//! Interactive treemap rendering and zoom handling.
//!
//! Only visible rectangles are emitted, small nodes below the pixel threshold
//! are aggregated into a remainder counter, and the layout is cached until the
//! viewport or the zoom root changes (FR-013…FR-017).

use eframe::egui::{self, Align2, FontId, Sense, Stroke, StrokeKind};
use freespace_core::{
    size::{Units, format_bytes},
    tree::{FsTree, NodeId},
};

use super::{
    colors::{self, CategoryColor, FileCategory, category_for},
    interaction::{self, Navigation},
    layout::{self, Rect, TreemapItem},
};

/// Node count above which the treemap aggregates the remainder.
const DEFAULT_MAX_ITEMS: usize = 2_000;

/// Cached layout of one zoom level.
#[derive(Debug)]
struct Cache {
    node: NodeId,
    width: u32,
    height: u32,
    items: Vec<TreemapItem>,
    hidden: usize,
}

/// Interactive treemap widget state.
#[derive(Debug)]
pub struct TreemapView {
    navigation: Navigation,
    /// Whether the category legend is visible.
    pub show_legend: bool,
    max_items: usize,
    cache: Option<Cache>,
}

impl Default for TreemapView {
    fn default() -> Self {
        Self {
            navigation: Navigation::default(),
            show_legend: true,
            max_items: DEFAULT_MAX_ITEMS,
            cache: None,
        }
    }
}

impl TreemapView {
    /// Resets the zoom to the scan root.
    pub fn reset(&mut self, root: Option<NodeId>) {
        self.navigation.reset(root);
        self.cache = None;
    }

    /// Draws the treemap and handles zoom interactions.
    pub fn ui(&mut self, ui: &mut egui::Ui, tree: &FsTree, units: Units) {
        let Some(scan_root) = tree.roots().next() else {
            ui.label(t!("treemap.empty").to_string());

            return;
        };
        if self.navigation.current().is_none() {
            self.reset(Some(scan_root));
        }
        let Some(current) = self.navigation.current() else {
            return;
        };

        self.toolbar(ui, tree, current);

        if self.show_legend {
            self.legend(ui);
        }

        let width = ui.available_width().max(64.0);
        let height = (ui.ctx().screen_rect().height() - 210.0).max(120.0);
        let (response, painter) = ui.allocate_painter(egui::vec2(width, height), Sense::click());
        let bounds = Rect::new(
            response.rect.left(),
            response.rect.top(),
            response.rect.width(),
            response.rect.height(),
        );

        let width = bounds.width.max(1.0) as u32;
        let height = bounds.height.max(1.0) as u32;
        let stale = self.cache.as_ref().is_none_or(|cache| {
            cache.node != current || cache.width != width || cache.height != height
        });
        if stale {
            let mut sizes: Vec<(NodeId, f64)> = tree
                .children(current)
                .map(|id| (id, tree.node(id).size.logical as f64))
                .filter(|(_, size)| *size > 0.0)
                .collect();
            sizes.sort_by(|left, right| right.1.total_cmp(&left.1));
            let hidden = sizes.len().saturating_sub(self.max_items);
            sizes.truncate(self.max_items);
            let items = layout::squarify(&sizes, bounds);
            self.cache = Some(Cache {
                node: current,
                width,
                height,
                items,
                hidden,
            });
        }
        let Some(cache) = &self.cache else {
            return;
        };

        painter.rect_filled(response.rect, 0.0, ui.visuals().extreme_bg_color);

        let dark = ui.visuals().dark_mode;
        for item in &cache.items {
            let node = tree.node(item.node);
            let color = if node.kind.is_directory() {
                colors::directory_color(dark)
            } else {
                category_for(tree.name(item.node)).color()
            };
            let rect = to_egui(item.rect.inset(1.0));
            if rect.width() <= 0.0 || rect.height() <= 0.0 {
                continue;
            }
            painter.rect_filled(rect, 1.0, color);
            painter.rect_stroke(
                rect,
                1.0,
                Stroke::new(0.5, ui.visuals().window_stroke.color),
                StrokeKind::Inside,
            );
            if rect.width() > 56.0 && rect.height() > 18.0 {
                painter.text(
                    rect.min + egui::vec2(3.0, 2.0),
                    Align2::LEFT_TOP,
                    tree.name(item.node),
                    FontId::proportional(11.0),
                    contrast_color(color),
                );
            }
        }

        if cache.hidden > 0 {
            painter.text(
                response.rect.left_bottom() + egui::vec2(6.0, -6.0),
                Align2::LEFT_BOTTOM,
                t!("treemap.more", count = cache.hidden).to_string(),
                FontId::proportional(11.0),
                ui.visuals().weak_text_color(),
            );
        }

        let hovered = response
            .hover_pos()
            .and_then(|pos| interaction::hit_test(&cache.items, pos.x, pos.y));
        let response = if let Some(node) = hovered {
            let tip = interaction::tooltip_for(tree, Some(current), node);
            response.on_hover_ui_at_pointer(|ui| {
                ui.strong(&tip.name);
                ui.label(&tip.path);
                ui.label(format!(
                    "{} · {}% · {} {}",
                    format_bytes(tip.size, units),
                    tip.share,
                    t!("treemap.files", count = tip.file_count),
                    t!("treemap.dirs", count = tip.dir_count)
                ));
                if !tip.accessible {
                    ui.colored_label(
                        ui.visuals().warn_fg_color,
                        t!("tree.access_denied").to_string(),
                    );
                }
            })
        } else {
            response
        };

        if response.double_clicked()
            && let Some(node) = hovered
            && self.navigation.enter(tree, node)
        {
            self.cache = None;
        }
        if response.secondary_clicked() && self.navigation.up(tree) {
            self.cache = None;
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, tree: &FsTree, current: NodeId) {
        let crumbs = self.navigation.breadcrumbs(tree);
        let mut target = None;
        let mut go_up = false;

        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    current != tree.roots().next().unwrap_or(current),
                    egui::Button::new("↑"),
                )
                .clicked()
            {
                go_up = true;
            }
            for (index, (id, name)) in crumbs.iter().enumerate() {
                if index > 0 {
                    ui.label("›");
                }
                if ui.selectable_label(*id == current, name).clicked() {
                    target = Some(*id);
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.checkbox(&mut self.show_legend, t!("treemap.legend").to_string());
            });
        });

        if go_up && self.navigation.up(tree) {
            self.cache = None;
        }
        if let Some(target) = target {
            self.navigation.jump_to(target);
            self.cache = None;
        }
    }

    fn legend(&self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            for category in FileCategory::ALL {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), Sense::hover());
                ui.painter().rect_filled(rect, 2.0, category.color());
                ui.label(t!(category.label_key()).to_string())
                    .on_hover_text(category.as_str());
            }
        });
    }
}

fn to_egui(rect: Rect) -> egui::Rect {
    egui::Rect::from_min_size(
        egui::pos2(rect.x, rect.y),
        egui::vec2(rect.width, rect.height),
    )
}

fn contrast_color(background: egui::Color32) -> egui::Color32 {
    let [red, green, blue, _] = background.to_array();
    let luminance = 0.299 * f32::from(red) + 0.587 * f32::from(green) + 0.114 * f32::from(blue);

    if luminance > 140.0 {
        egui::Color32::from_rgb(20, 22, 26)
    } else {
        egui::Color32::from_rgb(240, 242, 246)
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui;

    use super::{contrast_color, to_egui};
    use crate::treemap::layout::Rect;

    #[test]
    fn converts_layout_rectangles() {
        let rect = to_egui(Rect::new(10.0, 20.0, 30.0, 40.0));

        assert_eq!(rect.min, egui::pos2(10.0, 20.0));
        assert_eq!(rect.width(), 30.0);
        assert_eq!(rect.height(), 40.0);
    }

    #[test]
    fn picks_a_contrasting_text_colour() {
        assert_eq!(
            contrast_color(egui::Color32::WHITE),
            egui::Color32::from_rgb(20, 22, 26)
        );
        assert_eq!(
            contrast_color(egui::Color32::BLACK),
            egui::Color32::from_rgb(240, 242, 246)
        );
    }
}
