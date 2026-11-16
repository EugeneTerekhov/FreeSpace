//! Disk snapshot creation and comparison screen (FR-038…FR-041).

use eframe::egui;
use freespace_core::{
    Timestamp,
    size::{Units, format_bytes},
};
use freespace_report::{SnapshotDiff, SnapshotPayload, SnapshotSettings, compare, from_tree};
use freespace_scan::ScanResult;

/// State of the snapshots screen.
#[derive(Debug, Default)]
pub struct SnapshotsState {
    out: String,
    base: String,
    target: String,
    diff: Option<SnapshotDiff>,
    message: Option<String>,
    error: Option<String>,
}

impl SnapshotsState {
    /// Clears transient results after a new scan.
    pub fn reset(&mut self) {
        self.message = None;
        self.error = None;
    }

    /// Draws the screen.
    pub fn ui(&mut self, ui: &mut egui::Ui, scan: Option<&ScanResult>, units: Units) {
        ui.label(t!("snapshots.intro").to_string());
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label(t!("snapshots.output").to_string());
            ui.add(
                egui::TextEdit::singleline(&mut self.out)
                    .desired_width(360.0)
                    .hint_text("C:\\snapshots\\disk.fssnap"),
            );
            if ui
                .add_enabled(
                    scan.is_some(),
                    egui::Button::new(t!("snapshots.create").to_string()),
                )
                .clicked()
            {
                self.create(scan, units);
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.heading(t!("snapshots.compare").to_string());
        ui.horizontal(|ui| {
            ui.label(t!("snapshots.base").to_string());
            ui.text_edit_singleline(&mut self.base);
        });
        ui.horizontal(|ui| {
            ui.label(t!("snapshots.target").to_string());
            ui.text_edit_singleline(&mut self.target);
        });
        if ui.button(t!("snapshots.diff").to_string()).clicked() {
            self.compare();
        }

        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        if let Some(message) = &self.message {
            ui.label(message);
        }

        if let Some(diff) = &self.diff {
            ui.add_space(8.0);
            ui.separator();
            ui.label(
                t!(
                    "snapshots.summary",
                    added = diff.added.len(),
                    removed = diff.removed.len(),
                    changed = diff.changed.len()
                )
                .to_string(),
            );
            ui.label(format!(
                "{}: {}",
                t!("snapshots.total_growth"),
                signed(diff.total_growth_bytes, units)
            ));
            for entry in diff.growth_by_directory.iter().take(20) {
                ui.label(format!(
                    "{:>12}  {}",
                    signed(entry.delta, units),
                    entry.path
                ));
            }
        }
    }

    fn create(&mut self, scan: Option<&ScanResult>, units: Units) {
        self.error = None;
        self.message = None;
        let Some(scan) = scan else {
            self.error = Some(t!("scan.required").to_string());
            return;
        };
        if self.out.trim().is_empty() {
            self.error = Some(t!("snapshots.output_required").to_string());
            return;
        }

        let result = from_tree(
            &scan.tree,
            &scan.volume,
            &scan.session.roots,
            SnapshotSettings {
                units: units.as_str().to_owned(),
                follow_links: false,
                include_ads: false,
            },
            Timestamp::now(),
        )
        .and_then(|payload| {
            let checksum = payload.compute_checksum()?;
            payload.write(std::path::Path::new(self.out.trim()))?;
            Ok((payload, checksum))
        });

        match result {
            Ok((payload, _checksum)) => {
                self.message =
                    Some(t!("snapshots.created", count = payload.node_count).to_string());
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn compare(&mut self) {
        self.error = None;
        self.message = None;
        self.diff = None;
        let base = SnapshotPayload::read(std::path::Path::new(self.base.trim()));
        let target = SnapshotPayload::read(std::path::Path::new(self.target.trim()));
        match (base, target) {
            (Ok(base), Ok(target)) => match compare(&base, &target) {
                Ok(diff) => self.diff = Some(diff),
                Err(error) => self.error = Some(error.to_string()),
            },
            (Err(error), _) | (_, Err(error)) => self.error = Some(error.to_string()),
        }
    }

    /// Number of changes in the last comparison; used by tests.
    #[cfg(test)]
    #[must_use]
    pub fn changes(&self) -> usize {
        self.diff.as_ref().map_or(0, SnapshotDiff::change_count)
    }
}

fn signed(value: i64, units: Units) -> String {
    if value < 0 {
        format!("-{}", format_bytes(value.unsigned_abs(), units))
    } else {
        format!("+{}", format_bytes(value.unsigned_abs(), units))
    }
}

#[cfg(test)]
mod tests {
    use super::SnapshotsState;

    #[test]
    fn starts_without_a_comparison() {
        assert_eq!(SnapshotsState::default().changes(), 0);
    }
}
