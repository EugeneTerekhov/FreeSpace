//! File manager for paths beyond the legacy `MAX_PATH` limit (FR-051…FR-053).

use eframe::egui;
use freespace_core::size::{Units, format_bytes};
use freespace_ops::long_path_ops::{LongPathOutcome, copy_to, delete, move_to, rename};
use freespace_platform::paths;

/// State of the long-path manager.
#[derive(Debug, Default)]
pub struct LongPathsState {
    path: String,
    new_name: String,
    destination: String,
    confirm_delete: bool,
    outcome: Option<LongPathOutcome>,
    message: Option<String>,
    error: Option<String>,
}

impl LongPathsState {
    /// Draws the screen.
    pub fn ui(&mut self, ui: &mut egui::Ui, units: Units) {
        ui.label(t!("long_paths.intro").to_string());
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label(t!("long_paths.path").to_string());
            ui.add(
                egui::TextEdit::singleline(&mut self.path)
                    .desired_width(520.0)
                    .hint_text(r"C:\very\long\path\file.bin"),
            );
        });
        ui.horizontal(|ui| {
            ui.label(t!("long_paths.new_name").to_string());
            ui.text_edit_singleline(&mut self.new_name);
            if ui.button(t!("long_paths.rename").to_string()).clicked() {
                let name = self.new_name.trim().to_owned();
                self.run(|path| rename(path, &name));
            }
        });
        ui.horizontal(|ui| {
            ui.label(t!("long_paths.destination").to_string());
            ui.text_edit_singleline(&mut self.destination);
            if ui.button(t!("long_paths.move").to_string()).clicked() {
                let destination = self.destination.trim().to_owned();
                self.run(|path| move_to(path, std::path::Path::new(&destination)));
            }
            if ui.button(t!("long_paths.copy").to_string()).clicked() {
                let destination = self.destination.trim().to_owned();
                self.run(|path| copy_to(path, std::path::Path::new(&destination)));
            }
        });

        ui.add_space(8.0);
        ui.checkbox(
            &mut self.confirm_delete,
            t!("long_paths.confirm_delete").to_string(),
        );
        if ui
            .add_enabled(
                self.confirm_delete,
                egui::Button::new(t!("long_paths.delete").to_string()),
            )
            .clicked()
        {
            self.run(delete);
            self.confirm_delete = false;
        }

        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        if let Some(message) = &self.message {
            ui.label(message);
        }

        if let Some(outcome) = &self.outcome {
            ui.add_space(8.0);
            ui.separator();
            ui.label(format!(
                "{}: {} → {} ({})",
                outcome.operation.as_str(),
                paths::display(&outcome.from),
                outcome
                    .to
                    .as_deref()
                    .map_or_else(|| "-".to_owned(), paths::display),
                format_bytes(outcome.bytes, units)
            ));
        }
    }

    fn run(
        &mut self,
        operation: impl FnOnce(&std::path::Path) -> freespace_core::Result<LongPathOutcome>,
    ) {
        self.error = None;
        self.message = None;
        self.outcome = None;
        let path = std::path::PathBuf::from(self.path.trim());
        if path.as_os_str().is_empty() {
            self.error = Some(t!("long_paths.path_required").to_string());
            return;
        }

        match operation(&path) {
            Ok(outcome) => {
                if let Some(target) = &outcome.to {
                    self.path = paths::display(target);
                }
                self.message = Some(t!("long_paths.done").to_string());
                self.outcome = Some(outcome);
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LongPathsState;

    #[test]
    fn starts_empty_and_requires_delete_confirmation() {
        let state = LongPathsState::default();

        assert!(state.outcome.is_none());
        assert!(!state.confirm_delete);
    }
}
