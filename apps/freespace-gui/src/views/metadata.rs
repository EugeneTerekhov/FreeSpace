//! Metadata columns and filters (FR-061…FR-063).
//!
//! Extraction reads every supported file and can touch thousands of media and
//! executable headers, so it runs on a worker thread and reports its result
//! through a channel. Corrupted and unsupported files remain visible as `н/д`.

use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};

use eframe::egui;
use freespace_analysis::{
    MetadataCache, MetadataFilter, MetadataKind, MetadataRecord, MetadataStatus, supported_files,
};
use freespace_core::{
    size::{Units, format_bytes},
    tree::NodeKind,
};
use freespace_platform::paths;
use freespace_scan::ScanResult;

/// State of the metadata screen.
#[derive(Debug, Default)]
pub struct MetadataState {
    records: Vec<MetadataRecord>,
    receiver: Option<Receiver<Vec<MetadataRecord>>>,
    extracting: bool,
    error: Option<String>,
    kind: Option<MetadataKind>,
    min_width: u32,
    min_height: u32,
    min_bitrate: u32,
    min_duration: u64,
}

impl MetadataState {
    /// Clears results after a new scan.
    pub fn reset(&mut self) {
        self.records.clear();
        self.receiver = None;
        self.extracting = false;
        self.error = None;
    }

    /// Number of extracted records; used by tests.
    #[cfg(test)]
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Draws the screen.
    pub fn ui(&mut self, ui: &mut egui::Ui, result: &ScanResult, units: Units) {
        ui.label(t!("metadata.intro").to_string());
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.extracting,
                    egui::Button::new(t!("metadata.read").to_string()),
                )
                .clicked()
            {
                self.start(result);
            }
            if self.extracting {
                ui.spinner();
                ui.label(t!("metadata.reading").to_string());
                ui.ctx().request_repaint_after(Duration::from_millis(100));
            }
        });

        self.poll();

        ui.horizontal_wrapped(|ui| {
            ui.label(t!("metadata.kind").to_string());
            egui::ComboBox::from_id_salt("metadata_kind")
                .selected_text(match self.kind {
                    Some(kind) => kind.as_str().to_owned(),
                    None => t!("metadata.all").to_string(),
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.kind, None, t!("metadata.all").to_string());
                    for kind in [
                        MetadataKind::Exif,
                        MetadataKind::Audio,
                        MetadataKind::Video,
                        MetadataKind::Pe,
                    ] {
                        ui.selectable_value(&mut self.kind, Some(kind), kind.as_str());
                    }
                });
            ui.separator();
            ui.label(t!("metadata.min_width").to_string());
            ui.add(egui::DragValue::new(&mut self.min_width).range(0..=100_000));
            ui.label(t!("metadata.min_height").to_string());
            ui.add(egui::DragValue::new(&mut self.min_height).range(0..=100_000));
            ui.label(t!("metadata.min_bitrate").to_string());
            ui.add(egui::DragValue::new(&mut self.min_bitrate).range(0..=100_000));
            ui.label(t!("metadata.min_duration").to_string());
            ui.add(egui::DragValue::new(&mut self.min_duration).range(0..=u64::from(u32::MAX)));
        });

        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        if self.records.is_empty() {
            ui.add_space(8.0);
            ui.label(t!("metadata.empty").to_string());

            return;
        }

        let filter = MetadataFilter {
            kinds: self.kind.into_iter().collect(),
            min_width: (self.min_width > 0).then_some(self.min_width),
            min_height: (self.min_height > 0).then_some(self.min_height),
            min_bitrate_kbps: (self.min_bitrate > 0).then_some(self.min_bitrate),
            min_duration_secs: (self.min_duration > 0).then_some(self.min_duration),
            ..MetadataFilter::default()
        };
        let visible: Vec<&MetadataRecord> = self
            .records
            .iter()
            .filter(|record| filter.is_empty() || filter.matches(record))
            .collect();

        ui.label(format!("{} / {}", visible.len(), self.records.len()));
        ui.separator();
        egui::Grid::new("metadata_grid")
            .num_columns(6)
            .striped(true)
            .show(ui, |ui| {
                ui.strong(t!("metadata.path").to_string());
                ui.strong(t!("metadata.kind").to_string());
                ui.strong(t!("metadata.status").to_string());
                ui.strong(t!("metadata.size").to_string());
                ui.strong(t!("metadata.details").to_string());
                ui.end_row();

                for record in visible.iter().take(2_000) {
                    let node = &result.tree.node(record.node_id);
                    if node.kind == NodeKind::File {
                        ui.label(paths::display(&result.tree.path(record.node_id)));
                        ui.label(record.kind.as_str());
                        ui.label(status_label(record.status));
                        ui.label(format_bytes(node.size.logical, units));
                        ui.label(tags_summary(record));
                    }
                    ui.end_row();
                }
            });

        if visible.len() > 2_000 {
            ui.label(
                egui::RichText::new(format!("… {}", visible.len() - 2_000))
                    .color(ui.visuals().weak_text_color()),
            );
        }
    }

    fn start(&mut self, result: &ScanResult) {
        self.error = None;
        self.extracting = true;
        let files: Vec<(u32, PathBuf)> = supported_files(&result.tree);
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);

        if thread::Builder::new()
            .name("metadata".to_owned())
            .spawn(move || {
                let cache = MetadataCache::new();
                let records = cache.extract_many(&files);
                let _ = sender.send(records);
            })
            .is_err()
        {
            self.extracting = false;
            self.error = Some("cannot start the metadata worker".to_owned());
        }
    }

    fn poll(&mut self) {
        let Some(receiver) = &self.receiver else {
            return;
        };
        match receiver.try_recv() {
            Ok(records) => {
                self.records = records;
                self.receiver = None;
                self.extracting = false;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.receiver = None;
                self.extracting = false;
                self.error = Some("the metadata worker stopped unexpectedly".to_owned());
            }
        }
    }
}

fn status_label(status: MetadataStatus) -> String {
    match status {
        MetadataStatus::Ok => "OK".to_owned(),
        MetadataStatus::Unsupported | MetadataStatus::Corrupted => t!("metadata.none").to_string(),
    }
}

fn tags_summary(record: &MetadataRecord) -> String {
    if record.tags.is_empty() {
        return t!("metadata.none").to_string();
    }

    record
        .tags
        .iter()
        .take(4)
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use super::MetadataState;

    #[test]
    fn starts_empty_and_resets() {
        let mut state = MetadataState::default();
        assert_eq!(state.len(), 0);
        assert!(!state.extracting);

        state.min_width = 3840;
        state.reset();
        assert_eq!(state.len(), 0);
    }
}
