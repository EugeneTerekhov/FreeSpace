//! Report export dialog (FR-070…FR-073).

use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};

use eframe::egui;
use freespace_analysis::{
    DuplicateOptions, annotate, find_duplicates, group_by_extension, top_files,
};
use freespace_core::{Timestamp, size::Units, tree::FsTree};
use freespace_platform::paths;
use freespace_report::{Report, ReportFormat, ReportMeta, ReportVolume, SectionName, write_report};
use freespace_scan::ScanResult;

/// Top-file limit used by GUI exports.
const TOP_LIMIT: usize = 1_000;

/// State of the export screen.
#[derive(Debug)]
pub struct ExportState {
    format: ReportFormat,
    include: [bool; 5],
    output: String,
    receiver: Option<Receiver<Result<String, String>>>,
    exporting: bool,
    message: Option<String>,
    error: Option<String>,
}

impl Default for ExportState {
    fn default() -> Self {
        Self {
            format: ReportFormat::Csv,
            include: [true; 5],
            output: String::new(),
            receiver: None,
            exporting: false,
            message: None,
            error: None,
        }
    }
}

impl ExportState {
    /// Clears transient state after a new scan.
    pub fn reset(&mut self) {
        self.receiver = None;
        self.exporting = false;
        self.message = None;
        self.error = None;
    }

    /// Draws the screen.
    pub fn ui(&mut self, ui: &mut egui::Ui, result: &ScanResult, units: Units) {
        ui.label(t!("export.intro").to_string());
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label(t!("export.format").to_string());
            egui::ComboBox::from_id_salt("export_format")
                .selected_text(self.format.as_str())
                .show_ui(ui, |ui| {
                    for format in ReportFormat::ALL {
                        ui.selectable_value(&mut self.format, format, format.as_str());
                    }
                });
            ui.separator();
            ui.label(t!("export.output").to_string());
            ui.add(
                egui::TextEdit::singleline(&mut self.output)
                    .hint_text(format!("freespace-report.{}", self.format.extension()))
                    .desired_width(360.0),
            );
        });

        ui.add_space(6.0);
        ui.label(t!("export.include").to_string());
        ui.horizontal_wrapped(|ui| {
            for (index, section) in SectionName::ALL.into_iter().enumerate() {
                ui.checkbox(&mut self.include[index], section_label(section));
            }
        });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.exporting,
                    egui::Button::new(t!("export.export").to_string()),
                )
                .clicked()
            {
                self.start(result, units);
            }
            if self.exporting {
                ui.spinner();
                ui.label(t!("export.exporting").to_string());
                ui.ctx().request_repaint_after(Duration::from_millis(100));
            }
        });
        self.poll();

        if let Some(message) = &self.message {
            ui.label(message);
        }
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    }

    fn start(&mut self, result: &ScanResult, units: Units) {
        self.error = None;
        self.message = None;
        let path = PathBuf::from(self.output.trim());
        if path.as_os_str().is_empty() {
            self.error = Some(t!("export.output").to_string());
            return;
        }
        let include: Vec<SectionName> = SectionName::ALL
            .into_iter()
            .enumerate()
            .filter(|(index, _)| self.include[*index])
            .map(|(_, section)| section)
            .collect();
        if include.is_empty() {
            self.error = Some(t!("export.include").to_string());
            return;
        }

        let format = self.format;
        let tree = result.tree.clone();
        let volume = result.volume.clone();
        let roots = result.session.roots.clone();
        let session_id = result.session.id.to_string();
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        self.exporting = true;

        let spawn = thread::Builder::new()
            .name("export".to_owned())
            .spawn(move || {
                let outcome = build_and_write(
                    tree, volume, roots, session_id, include, format, &path, units,
                )
                .map(|_| paths::display(&path))
                .map_err(|error| error.to_string());
                let _ = sender.send(outcome);
            });

        if spawn.is_err() {
            self.exporting = false;
            self.error = Some("cannot start the export worker".to_owned());
        }
    }

    fn poll(&mut self) {
        let Some(receiver) = &self.receiver else {
            return;
        };
        match receiver.try_recv() {
            Ok(Ok(path)) => {
                self.message = Some(t!("export.done", path = path).to_string());
                self.receiver = None;
                self.exporting = false;
            }
            Ok(Err(error)) => {
                self.error = Some(error);
                self.receiver = None;
                self.exporting = false;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.error = Some("the export worker stopped unexpectedly".to_owned());
                self.receiver = None;
                self.exporting = false;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_and_write(
    mut tree: FsTree,
    volume: freespace_core::model::Volume,
    roots: Vec<PathBuf>,
    session_id: String,
    include: Vec<SectionName>,
    format: ReportFormat,
    path: &Path,
    units: Units,
) -> freespace_core::Result<()> {
    let owner_scan = if include.contains(&SectionName::Owners) {
        Some(annotate(&mut tree)?)
    } else {
        None
    };
    let top = include
        .contains(&SectionName::Top)
        .then(|| top_files(&tree, TOP_LIMIT));
    let types = include
        .contains(&SectionName::Types)
        .then(|| group_by_extension(&tree));
    let duplicates = if include.contains(&SectionName::Duplicates) {
        Some(find_duplicates(&tree, DuplicateOptions::default(), None)?)
    } else {
        None
    };

    let total = tree
        .roots()
        .map(|root| tree.node(root).size)
        .sum::<freespace_core::size::SizeMetrics>();
    let meta = ReportMeta::new(Timestamp::now(), env!("CARGO_PKG_VERSION"))
        .with_volume(ReportVolume::from_volume(&volume))
        .with_roots(roots.iter().map(|root| paths::display(root)))
        .with_session(session_id)
        .with_units(units.as_str());
    let mut report = Report::new(meta, total)
        .with_include(include)
        .with_tree(&tree);
    if let Some(top) = &top {
        report = report.with_top(top);
    }
    if let Some(types) = &types {
        report = report.with_types(types);
    }
    if let Some(search) = &duplicates {
        report = report.with_duplicates(&search.groups);
    }
    if let Some(scan) = &owner_scan {
        report = report.with_owners(&scan.summaries, &scan.table);
    }

    write_report(&report, format, path)
}

fn section_label(section: SectionName) -> String {
    match section {
        SectionName::Tree => t!("export.section.tree").to_string(),
        SectionName::Top => t!("export.section.top").to_string(),
        SectionName::Types => t!("export.section.types").to_string(),
        SectionName::Duplicates => t!("export.section.duplicates").to_string(),
        SectionName::Owners => t!("export.section.owners").to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::ExportState;

    #[test]
    fn defaults_to_csv_with_every_section() {
        let state = ExportState::default();

        assert_eq!(state.format, freespace_report::ReportFormat::Csv);
        assert_eq!(state.include, [true; 5]);
        assert!(!state.exporting);
    }
}
