//! Archive contents without unpacking (FR-066…FR-069).

use std::path::PathBuf;

use eframe::egui;
use freespace_analysis::{ArchiveInspection, ArchiveLimits, find_archives, inspect};
use freespace_core::size::{Units, format_bytes};
use freespace_platform::paths;

/// Maximum number of archives opened from one folder.
const MAX_ARCHIVES: usize = 500;

/// State of the archive screen.
#[derive(Debug, Default)]
pub struct ArchivesState {
    path: String,
    export_path: String,
    inspections: Vec<ArchiveInspection>,
    message: Option<String>,
    error: Option<String>,
}

impl ArchivesState {
    /// Clears results after a new scan.
    pub fn reset(&mut self) {
        self.inspections.clear();
        self.message = None;
        self.error = None;
    }

    /// Number of inspected archives; used by tests.
    #[cfg(test)]
    #[must_use]
    pub fn len(&self) -> usize {
        self.inspections.len()
    }

    /// Draws the screen.
    pub fn ui(&mut self, ui: &mut egui::Ui, units: Units) {
        ui.label(t!("archives.intro").to_string());
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label(t!("archives.path").to_string());
            ui.add(
                egui::TextEdit::singleline(&mut self.path)
                    .hint_text(r"C:\Downloads\archive.zip")
                    .desired_width(420.0),
            );
            if ui.button(t!("archives.inspect").to_string()).clicked() {
                self.inspect(units);
            }
        });

        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        if let Some(message) = &self.message {
            ui.label(message);
        }
        if self.inspections.is_empty() {
            ui.label(t!("archives.empty").to_string());

            return;
        }

        ui.separator();
        ui.horizontal(|ui| {
            ui.label(t!("archives.export_path").to_string());
            ui.add(
                egui::TextEdit::singleline(&mut self.export_path)
                    .hint_text("archive-list.csv")
                    .desired_width(320.0),
            );
            if ui.button(t!("archives.export").to_string()).clicked() {
                self.export();
            }
        });
        ui.separator();

        for inspection in &self.inspections {
            let header = format!(
                "{} · {} · {}",
                paths::display(&inspection.archive),
                inspection.format.as_str(),
                format_bytes(inspection.total_size, units)
            );
            egui::CollapsingHeader::new(header)
                .id_salt(inspection.archive.to_string_lossy().into_owned())
                .default_open(self.inspections.len() == 1)
                .show(ui, |ui| {
                    ui.label(t!("archives.entries", count = inspection.entries.len()).to_string());
                    for entry in inspection.largest(200) {
                        ui.horizontal(|ui| {
                            ui.label(format!("{:>12}", format_bytes(entry.size, units)));
                            ui.label(if entry.is_directory {
                                format!("{}/", entry.internal_path)
                            } else {
                                entry.internal_path.clone()
                            });
                            if entry.encrypted {
                                ui.label("🔒");
                            }
                            ui.label(format!("{:.1}%", entry.share * 100.0));
                        });
                    }
                    for warning in &inspection.warnings {
                        ui.colored_label(ui.visuals().warn_fg_color, warning);
                    }
                });
        }
    }

    fn inspect(&mut self, units: Units) {
        self.error = None;
        self.message = None;
        self.inspections.clear();
        let path = PathBuf::from(self.path.trim());
        if path.as_os_str().is_empty() {
            self.error = Some(t!("archives.empty").to_string());
            return;
        }
        let limits = ArchiveLimits::default();

        if path.is_dir() {
            let archives = find_archives(&path, MAX_ARCHIVES);
            let mut failures = Vec::new();
            for archive in archives {
                match inspect(&archive, limits) {
                    Ok(inspection) => self.inspections.push(inspection),
                    Err(error) => failures.push(format!("{}: {error}", paths::display(&archive))),
                }
            }
            if !failures.is_empty() {
                self.error = Some(failures.join("\n"));
            }
        } else {
            match inspect(&path, limits) {
                Ok(inspection) => self.inspections.push(inspection),
                Err(error) => self.error = Some(error.to_string()),
            }
        }
        if self.inspections.is_empty() && self.error.is_none() {
            self.message = Some(format!(
                "{} {}",
                t!("archives.total", size = format_bytes(0, units)),
                t!("archives.empty")
            ));
        }
    }

    fn export(&mut self) {
        self.error = None;
        self.message = None;
        let path = PathBuf::from(self.export_path.trim());
        if path.as_os_str().is_empty() {
            self.error = Some(t!("archives.export_path").to_string());
            return;
        }
        match freespace_report::archive_list::write(&self.inspections, &path) {
            Ok(()) => {
                self.message =
                    Some(t!("archives.exported", path = paths::display(&path)).to_string());
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ArchivesState;

    #[test]
    fn starts_empty_and_resets() {
        let mut state = ArchivesState::default();
        assert_eq!(state.len(), 0);

        state.error = Some("boom".to_owned());
        state.reset();
        assert!(state.error.is_none());
    }
}
