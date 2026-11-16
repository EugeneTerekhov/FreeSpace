//! Per-owner report and tree filter (FR-064, FR-065).
//!
//! Ownership queries run on a worker thread. The completed assignments are
//! handed back to the shell, which writes them into the analysed tree so the
//! existing owner filter in [`freespace_core::query`] works unchanged.

use std::{
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};

use eframe::egui;
use freespace_analysis::{OwnerScan, OwnerSummary, UNKNOWN_OWNER, annotate};
use freespace_core::{
    Error,
    size::{Units, format_bytes},
    tree::NodeId,
};
use freespace_scan::ScanResult;

/// Result sent by the owner worker.
#[derive(Debug)]
struct OwnerWorkerResult {
    scan: OwnerScan,
    assignments: Vec<(NodeId, Option<u32>)>,
}

/// State of the owners screen.
#[derive(Debug, Default)]
pub struct OwnersState {
    summaries: Vec<OwnerSummary>,
    receiver: Option<Receiver<Result<OwnerWorkerResult, Error>>>,
    computing: bool,
    selected: Option<u32>,
    pending: Option<Vec<(NodeId, Option<u32>)>>,
    error: Option<String>,
}

impl OwnersState {
    /// Clears results after a new scan.
    pub fn reset(&mut self) {
        self.summaries.clear();
        self.receiver = None;
        self.computing = false;
        self.selected = None;
        self.pending = None;
        self.error = None;
    }

    /// Takes owner assignments that must be written into the analysed tree.
    pub fn take_pending(&mut self) -> Option<Vec<(NodeId, Option<u32>)>> {
        self.pending.take()
    }

    /// Reports an error from the shell (for example a failed tree update).
    pub fn set_error(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
    }

    /// Number of owner rows; used by tests.
    #[cfg(test)]
    #[must_use]
    pub fn len(&self) -> usize {
        self.summaries.len()
    }

    /// Draws the screen; returns the owner selected for tree filtering.
    pub fn ui(&mut self, ui: &mut egui::Ui, result: &ScanResult, units: Units) -> Option<u32> {
        ui.label(t!("owners.intro").to_string());
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.computing,
                    egui::Button::new(t!("owners.analyze").to_string()),
                )
                .clicked()
            {
                self.start(result);
            }
            if self.computing {
                ui.spinner();
                ui.label(t!("owners.analyzing").to_string());
                ui.ctx().request_repaint_after(Duration::from_millis(100));
            }
        });
        self.poll();

        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        if self.summaries.is_empty() {
            ui.label(t!("owners.empty").to_string());

            return None;
        }

        let total: u64 = self
            .summaries
            .iter()
            .map(|summary| summary.total_bytes)
            .fold(0, u64::saturating_add);

        let mut filter = None;
        ui.horizontal(|ui| {
            ui.label(t!("owners.filter_tree").to_string());
            let selected_text = self
                .selected
                .and_then(|owner_id| {
                    self.summaries
                        .iter()
                        .find(|summary| summary.owner_id == owner_id)
                        .map(|summary| summary.display_name.clone())
                })
                .unwrap_or_else(|| t!("metadata.all").to_string());
            egui::ComboBox::from_id_salt("owners_filter")
                .selected_text(selected_text)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.selected, None, t!("metadata.all").to_string());
                    for summary in &self.summaries {
                        ui.selectable_value(
                            &mut self.selected,
                            Some(summary.owner_id),
                            format!("{} ({})", summary.display_name, summary.file_count),
                        );
                    }
                });
            if ui
                .add_enabled(self.selected.is_some(), egui::Button::new("→"))
                .clicked()
            {
                filter = self.selected;
            }
        });

        ui.separator();
        egui::Grid::new("owners_grid")
            .num_columns(5)
            .striped(true)
            .show(ui, |ui| {
                ui.strong(t!("owners.owner").to_string());
                ui.strong(t!("owners.sid").to_string());
                ui.strong(t!("owners.files").to_string());
                ui.strong(t!("owners.size").to_string());
                ui.strong(t!("owners.share").to_string());
                ui.end_row();

                for summary in &self.summaries {
                    ui.label(&summary.display_name);
                    ui.label(if summary.sid.is_empty() {
                        t!("metadata.none").to_string()
                    } else {
                        summary.sid.clone()
                    });
                    ui.label(summary.file_count.to_string());
                    ui.label(format_bytes(summary.total_bytes, units));
                    ui.label(format!("{:.1}%", summary.share(total) * 100.0));
                    ui.end_row();
                }
            });

        if self
            .summaries
            .iter()
            .any(|summary| summary.display_name == UNKNOWN_OWNER)
        {
            ui.label(t!("owners.unknown").to_string());
        }

        filter
    }

    fn start(&mut self, result: &ScanResult) {
        self.error = None;
        self.computing = true;
        let mut tree = result.tree.clone();
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);

        if thread::Builder::new()
            .name("owners".to_owned())
            .spawn(move || {
                let outcome = annotate(&mut tree).map(|scan| {
                    let assignments = (0..tree.len())
                        .filter_map(|index| NodeId::try_from(index).ok())
                        .map(|node_id| (node_id, tree.node(node_id).owner_id))
                        .collect();
                    OwnerWorkerResult { scan, assignments }
                });
                let _ = sender.send(outcome);
            })
            .is_err()
        {
            self.computing = false;
            self.error = Some("cannot start the owner worker".to_owned());
        }
    }

    fn poll(&mut self) {
        let Some(receiver) = &self.receiver else {
            return;
        };
        match receiver.try_recv() {
            Ok(Ok(result)) => {
                self.summaries = result.scan.summaries;
                self.pending = Some(result.assignments);
                self.receiver = None;
                self.computing = false;
            }
            Ok(Err(error)) => {
                self.receiver = None;
                self.computing = false;
                self.error = Some(error.to_string());
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.receiver = None;
                self.computing = false;
                self.error = Some("the owner worker stopped unexpectedly".to_owned());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::OwnersState;

    #[test]
    fn starts_empty_and_resets() {
        let mut state = OwnersState::default();
        assert_eq!(state.len(), 0);
        assert!(!state.computing);

        state.error = Some("boom".to_owned());
        state.reset();
        assert!(state.error.is_none());
    }
}
