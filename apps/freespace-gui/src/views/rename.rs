//! Bulk rename screen with preview, conflicts and rollback (FR-054…FR-056).

use std::path::PathBuf;

use eframe::egui;
use freespace_core::Result;
use freespace_ops::{
    CaseTransform, RenamePlan, RenameRule, Store,
    rename_plan::{apply_plan, build_plan},
    store,
};
use freespace_platform::paths;

/// State of the bulk rename screen.
#[derive(Debug)]
pub struct RenameState {
    paths_text: String,
    regex: String,
    replacement: String,
    case: CaseTransform,
    plan: Option<RenamePlan>,
    confirmed: bool,
    message: Option<String>,
    error: Option<String>,
}

impl Default for RenameState {
    fn default() -> Self {
        Self {
            paths_text: String::new(),
            regex: String::new(),
            replacement: String::new(),
            case: CaseTransform::Preserve,
            plan: None,
            confirmed: false,
            message: None,
            error: None,
        }
    }
}

impl RenameState {
    /// Draws the screen.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.label(t!("rename.intro").to_string());
        ui.add_space(8.0);

        ui.label(t!("rename.files").to_string());
        ui.add(
            egui::TextEdit::multiline(&mut self.paths_text)
                .desired_rows(4)
                .desired_width(f32::INFINITY)
                .hint_text(r"C:\photos\IMG_0001.jpg"),
        );

        ui.horizontal_wrapped(|ui| {
            ui.label(t!("rename.pattern").to_string());
            ui.text_edit_singleline(&mut self.regex);
            ui.label(t!("rename.replacement").to_string());
            ui.text_edit_singleline(&mut self.replacement);
            egui::ComboBox::from_id_salt("rename_case")
                .selected_text(self.case.as_str())
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.case,
                        CaseTransform::Preserve,
                        CaseTransform::Preserve.as_str(),
                    );
                    ui.selectable_value(
                        &mut self.case,
                        CaseTransform::Lower,
                        CaseTransform::Lower.as_str(),
                    );
                    ui.selectable_value(
                        &mut self.case,
                        CaseTransform::Upper,
                        CaseTransform::Upper.as_str(),
                    );
                });
        });

        ui.add_space(8.0);
        if ui.button(t!("rename.preview").to_string()).clicked() {
            self.preview();
        }

        if let Some(plan) = &self.plan {
            ui.separator();
            ui.label(
                t!(
                    "rename.plan",
                    count = plan.items.len(),
                    conflicts = plan.conflicts.len()
                )
                .to_string(),
            );
            for item in &plan.items {
                ui.label(format!(
                    "{} → {}",
                    paths::display(&item.from),
                    paths::display(&item.to)
                ));
            }
            for conflict in &plan.conflicts {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    format!("{}: {}", paths::display(&conflict.path), conflict.reason),
                );
            }
        }

        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        if let Some(message) = &self.message {
            ui.label(message);
        }

        ui.add_space(8.0);
        ui.checkbox(&mut self.confirmed, t!("rename.confirm").to_string());
        let can_apply = self
            .plan
            .as_ref()
            .is_some_and(|plan| plan.is_clear() && !plan.items.is_empty());
        if ui
            .add_enabled(
                self.confirmed && can_apply,
                egui::Button::new(t!("rename.apply").to_string()),
            )
            .clicked()
        {
            self.apply();
        }
    }

    fn target_paths(&self) -> Vec<PathBuf> {
        self.paths_text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(PathBuf::from)
            .collect()
    }

    fn preview(&mut self) {
        self.error = None;
        self.message = None;
        self.plan = None;
        let result = RenameRule::new(&self.regex, &self.replacement, self.case)
            .and_then(|rule| build_plan(&self.target_paths(), &rule));

        match result {
            Ok(plan) => self.plan = Some(plan),
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn apply(&mut self) {
        self.error = None;
        self.message = None;
        let Some(plan) = self.plan.clone() else {
            return;
        };

        let result: Result<_> = (|| {
            let path = store::default_path()?;
            let store = Store::open(&path)?;
            apply_plan(&plan, Some(&store))
        })();

        match result {
            Ok(outcome) => {
                self.message = Some(t!("rename.done", count = outcome.renamed).to_string());
                self.confirmed = false;
                self.plan = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    /// Number of previewed renames; used by tests.
    #[cfg(test)]
    #[must_use]
    pub fn preview_count(&self) -> usize {
        self.plan.as_ref().map_or(0, RenamePlan::change_count)
    }
}

#[cfg(test)]
mod tests {
    use super::RenameState;

    #[test]
    fn starts_without_a_plan() {
        assert_eq!(RenameState::default().preview_count(), 0);
    }
}
