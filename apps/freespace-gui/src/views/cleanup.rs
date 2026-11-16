//! Cleanup recommendations and empty-directory screen (FR-034…FR-037).

use std::collections::HashSet;

use eframe::egui;
use freespace_analysis::{CleanupCandidate, RecommendOptions, recommend};
use freespace_core::{
    Timestamp,
    size::{Units, format_bytes},
};
use freespace_ops::{
    DeleteMode, DeleteOptions, Store, delete::delete_paths, quarantine::Quarantine, store,
};
use freespace_platform::{paths, volumes};
use freespace_scan::{EmptyDir, ScanResult, find_empty_dirs};

use super::confirm::{Confirmation, DeletionDialog};

/// State of the cleanup screen.
#[derive(Debug, Default)]
pub struct CleanupState {
    older_than: u32,
    candidates: Vec<CleanupCandidate>,
    empty_dirs: Vec<EmptyDir>,
    selected: HashSet<usize>,
    dialog: Option<DeletionDialog>,
    message: Option<String>,
    error: Option<String>,
}

impl CleanupState {
    /// Clears results after a new scan.
    pub fn reset(&mut self) {
        self.candidates.clear();
        self.empty_dirs.clear();
        self.selected.clear();
        self.dialog = None;
        self.message = None;
        self.error = None;
    }

    /// Draws the screen for the current scan.
    pub fn ui(&mut self, ui: &mut egui::Ui, result: &ScanResult, units: Units) {
        ui.label(t!("cleanup.intro").to_string());
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label(t!("cleanup.older_than").to_string());
            ui.add(egui::DragValue::new(&mut self.older_than).range(0..=3650));
            if ui.button(t!("cleanup.analyze").to_string()).clicked() {
                self.analyze(result);
            }
            if ui.button(t!("cleanup.find_empty").to_string()).clicked() {
                self.empty_dirs = find_empty_dirs(&result.tree, false);
                self.message =
                    Some(t!("cleanup.empty_count", count = self.empty_dirs.len()).to_string());
            }
        });

        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        if let Some(message) = &self.message {
            ui.label(message);
        }

        ui.add_space(8.0);
        ui.separator();
        ui.heading(t!("cleanup.recommendations").to_string());
        if self.candidates.is_empty() {
            ui.label(t!("cleanup.none").to_string());
        } else {
            let mut reclaimable = 0u64;
            for (index, candidate) in self.candidates.iter().enumerate() {
                reclaimable = reclaimable.saturating_add(candidate.reclaimable_bytes);
                ui.horizontal(|ui| {
                    let mut selected = self.selected.contains(&index);
                    if ui.checkbox(&mut selected, "").changed() {
                        if selected {
                            self.selected.insert(index);
                        } else {
                            self.selected.remove(&index);
                        }
                    }
                    ui.label(format!(
                        "{:>12}  {:<9} {:<6}  {}",
                        format_bytes(candidate.reclaimable_bytes, units),
                        candidate.category.as_str(),
                        candidate.risk.as_str(),
                        paths::display(&candidate.path)
                    ));
                });
                ui.label(format!("    {}", candidate.reason));
            }
            ui.label(t!("cleanup.total", size = format_bytes(reclaimable, units)).to_string());
            if ui
                .add_enabled(
                    !self.selected.is_empty(),
                    egui::Button::new(t!("cleanup.delete_selected").to_string()),
                )
                .clicked()
            {
                let targets: Vec<_> = self
                    .candidates
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| self.selected.contains(index))
                    .map(|(_, candidate)| candidate.path.clone())
                    .collect();
                self.open_dialog(targets);
            }
        }

        if !self.empty_dirs.is_empty() {
            ui.add_space(8.0);
            ui.separator();
            ui.heading(t!("cleanup.empty_dirs").to_string());
            for entry in &self.empty_dirs {
                ui.label(paths::display(&entry.path));
            }
            if ui.button(t!("cleanup.delete_empty").to_string()).clicked() {
                let targets = self
                    .empty_dirs
                    .iter()
                    .map(|entry| entry.path.clone())
                    .collect();
                self.open_dialog(targets);
            }
        }

        let mut pending = None;
        let mut cancel = false;
        if let Some(dialog) = &mut self.dialog {
            match dialog.ui(ui, units) {
                Confirmation::Confirmed => {
                    let mode = dialog.mode();
                    let targets: Vec<_> = dialog
                        .preview()
                        .items
                        .iter()
                        .map(|item| item.path.clone())
                        .collect();
                    pending = Some((targets, mode));
                }
                Confirmation::Cancelled => cancel = true,
                Confirmation::Pending => {}
            }
        }
        if cancel {
            self.dialog = None;
        }
        if let Some((targets, mode)) = pending {
            self.execute(targets, mode);
        }
    }

    fn open_dialog(&mut self, targets: Vec<std::path::PathBuf>) {
        self.error = None;
        let available = targets
            .first()
            .and_then(|path| volumes::describe_volume(path).ok())
            .map(|volume| volume.free_bytes);
        match freespace_ops::preview_delete(&targets, available) {
            Ok(preview) => self.dialog = Some(DeletionDialog::new(preview)),
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn execute(&mut self, targets: Vec<std::path::PathBuf>, mode: DeleteMode) {
        self.dialog = None;
        self.error = None;
        self.message = None;

        let result = (|| {
            let path = store::default_path()?;
            let store = Store::open(&path)?;
            let quarantine = if mode == DeleteMode::Quarantine {
                Some(Quarantine::new(Quarantine::default_root(false)?)?)
            } else {
                None
            };
            let options = DeleteOptions {
                mode,
                reason: "cleanup recommendation".to_owned(),
                retention_days: 30,
                available_bytes: None,
                allow_protected: false,
            };
            delete_paths(&store, quarantine.as_ref(), &targets, &options, true)
        })();

        match result {
            Ok(outcome) => {
                self.message = Some(
                    t!(
                        "cleanup.deleted",
                        count = outcome.summary.succeeded,
                        failed = outcome.summary.failed
                    )
                    .to_string(),
                );
                self.candidates
                    .retain(|candidate| !targets.iter().any(|path| path == &candidate.path));
                self.empty_dirs
                    .retain(|entry| !targets.iter().any(|path| path == &entry.path));
                self.selected.clear();
                if let Some(error) = outcome.first_error() {
                    self.error = Some(error.to_owned());
                }
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn analyze(&mut self, result: &ScanResult) {
        self.error = None;
        self.candidates = recommend(
            &result.tree,
            RecommendOptions {
                now: Timestamp::now(),
                older_than_days: self.older_than,
            },
        );
        self.selected.clear();
        self.message = Some(t!("cleanup.found", count = self.candidates.len()).to_string());
    }

    /// Number of candidates in the last analysis.
    #[cfg(test)]
    #[must_use]
    pub fn candidate_count(&self) -> usize {
        self.candidates.len()
    }
}

#[cfg(test)]
mod tests {
    use super::CleanupState;

    #[test]
    fn starts_empty() {
        let state = CleanupState::default();

        assert_eq!(state.candidate_count(), 0);
        assert!(state.selected.is_empty());
    }
}
