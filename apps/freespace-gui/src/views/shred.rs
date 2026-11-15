//! Shredder screen: explicit confirmation, SSD warning and journal.
//!
//! The screen never destroys anything on a single click: the user states the
//! files, inspects the media type, ticks the confirmation box and only then can
//! press the destructive button (FR-030, FR-031).

use std::path::PathBuf;

use eframe::egui;
use freespace_core::size::Units;
use freespace_ops::{
    ShredOptions, Store,
    operation::ShredPattern,
    shred::{self, ShredOutcome},
    store,
};
use freespace_platform::{paths, trim::StorageKind};

/// State of the shredder screen.
#[derive(Debug)]
pub struct ShredState {
    paths: String,
    passes: u32,
    pattern: ShredPattern,
    storage: Option<StorageKind>,
    confirm: bool,
    outcomes: Vec<ShredOutcome>,
    message: Option<String>,
    error: Option<String>,
}

impl Default for ShredState {
    fn default() -> Self {
        Self {
            paths: String::new(),
            passes: 1,
            pattern: ShredPattern::Zeros,
            storage: None,
            confirm: false,
            outcomes: Vec::new(),
            message: None,
            error: None,
        }
    }
}

impl ShredState {
    /// Draws the screen.
    pub fn ui(&mut self, ui: &mut egui::Ui, units: Units) {
        ui.label(t!("shred.intro").to_string());
        ui.colored_label(
            ui.visuals().warn_fg_color,
            t!("shred.irreversible").to_string(),
        );
        ui.add_space(8.0);

        ui.horizontal_wrapped(|ui| {
            ui.label(t!("shred.passes").to_string());
            ui.add(egui::DragValue::new(&mut self.passes).range(1..=35));
            ui.label(t!("shred.pattern").to_string());
            egui::ComboBox::from_id_salt("shred_pattern")
                .selected_text(self.pattern.as_str())
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.pattern,
                        ShredPattern::Zeros,
                        ShredPattern::Zeros.as_str(),
                    );
                    ui.selectable_value(
                        &mut self.pattern,
                        ShredPattern::Random,
                        ShredPattern::Random.as_str(),
                    );
                });
        });

        ui.add_space(8.0);
        ui.label(t!("shred.files").to_string());
        ui.add(
            egui::TextEdit::multiline(&mut self.paths)
                .desired_rows(4)
                .desired_width(f32::INFINITY)
                .hint_text(t!("shred.files_hint").to_string()),
        );

        ui.add_space(8.0);
        if ui.button(t!("shred.inspect").to_string()).clicked() {
            self.inspect();
        }

        if let Some(storage) = self.storage {
            ui.add_space(4.0);
            if storage.needs_warning() {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    t!("shred.ssd_warning").to_string(),
                );
            } else {
                ui.label(format!("{}: {}", t!("shred.storage"), storage.as_str()));
            }
        }

        ui.add_space(8.0);
        ui.checkbox(&mut self.confirm, t!("shred.confirm").to_string());
        let files = self.files();
        if ui
            .add_enabled(
                self.confirm && !files.is_empty(),
                egui::Button::new(t!("shred.destroy").to_string()),
            )
            .clicked()
        {
            self.destroy(&files);
        }

        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        if let Some(message) = &self.message {
            ui.label(message);
        }

        if !self.outcomes.is_empty() {
            ui.add_space(8.0);
            ui.separator();
            for outcome in &self.outcomes {
                ui.label(format!(
                    "{} — {} ({}, {})",
                    paths::display(&outcome.path),
                    freespace_core::size::format_bytes(outcome.size, units),
                    outcome.pattern.as_str(),
                    outcome.storage.as_str()
                ));
                for warning in &outcome.warnings {
                    ui.colored_label(ui.visuals().warn_fg_color, warning.clone());
                }
            }
        }
    }

    fn files(&self) -> Vec<PathBuf> {
        self.paths
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(PathBuf::from)
            .collect()
    }

    fn inspect(&mut self) {
        self.error = None;
        self.storage = None;
        let files = self.files();
        let Some(first) = files.first() else {
            self.error = Some(t!("shred.no_files").to_string());
            return;
        };
        match freespace_platform::trim::storage_kind(first) {
            Ok(storage) => self.storage = Some(storage),
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn destroy(&mut self, files: &[PathBuf]) {
        self.error = None;
        self.message = None;
        self.confirm = false;
        self.outcomes.clear();

        let options = ShredOptions {
            passes: self.passes,
            pattern: self.pattern,
        };
        for path in files {
            match shred::shred(path, options, true) {
                Ok(outcome) => self.outcomes.push(outcome),
                Err(error) => {
                    self.error = Some(format!("{}: {error}", paths::display(path)));
                    return;
                }
            }
        }

        if !self.outcomes.is_empty() {
            self.journal();
            self.message = Some(t!("shred.done", count = self.outcomes.len()).to_string());
        }
    }

    fn journal(&mut self) {
        let Ok(path) = store::default_path() else {
            return;
        };
        let Ok(database) = Store::open(&path) else {
            return;
        };
        let user = std::env::var("USERNAME").unwrap_or_else(|_| "unknown".to_owned());
        if let Err(error) = shred::record(&database, &self.outcomes, &user) {
            self.error = Some(error.to_string());
        }
    }

    /// Number of destroyed files in the last run.
    #[cfg(test)]
    #[must_use]
    pub fn destroyed(&self) -> usize {
        self.outcomes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::ShredState;

    #[test]
    fn starts_empty_and_requires_explicit_input() {
        let state = ShredState::default();

        assert_eq!(state.destroyed(), 0);
        assert!(!state.confirm);
        assert!(!freespace_ops::shred::SSD_WARNING.is_empty());
    }

    #[test]
    fn splits_the_file_list() {
        let state = ShredState {
            paths: "  C:\\a.txt \r\n\r\nC:\\b.txt\n".to_owned(),
            ..ShredState::default()
        };

        assert_eq!(state.files().len(), 2);
    }
}
