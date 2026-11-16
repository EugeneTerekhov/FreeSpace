//! Drive dashboard screen (US22).
//!
//! Shows every volume with its occupied/free/total space, file system and the
//! occupancy trend reconstructed from stored snapshots. The *Analyze* button
//! returns the selected mount point so the shell can start a scan and switch to
//! the tree.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use eframe::egui;
use freespace_core::{
    Timestamp, Uuid,
    dashboard::{Dashboard, SnapshotPoint},
    model::{Volume, VolumeStatus},
    size::{Units, format_bytes},
};
use freespace_platform::volumes;
use freespace_report::SnapshotPayload;

/// State of the dashboard screen.
#[derive(Debug, Default)]
pub struct DashboardState {
    dashboard: Option<Dashboard>,
    snapshots_dir: String,
    error: Option<String>,
    message: Option<String>,
    loaded: bool,
}

impl DashboardState {
    /// Loads the volume summary and snapshot trends.
    pub fn refresh(&mut self) {
        self.loaded = true;
        match volumes::dashboard() {
            Ok(mut dashboard) => {
                let trends = self.load_trends();
                dashboard.attach_trends(&trends);
                self.message = Some(format!(
                    "{} {}",
                    t!("dashboard.volume"),
                    dashboard.volumes.len()
                ));
                self.dashboard = Some(dashboard);
                self.error = None;
            }
            Err(error) => {
                self.error = Some(error.to_string());
            }
        }
    }

    /// Draws the screen; returns the mount point whose analysis was requested.
    pub fn ui(&mut self, ui: &mut egui::Ui, units: Units) -> Option<PathBuf> {
        if !self.loaded {
            self.refresh();
        }

        ui.label(t!("dashboard.intro").to_string());
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            if ui.button(t!("dashboard.refresh").to_string()).clicked() {
                self.refresh();
            }
            ui.label(t!("snapshots.output").to_string());
            ui.add(
                egui::TextEdit::singleline(&mut self.snapshots_dir)
                    .desired_width(320.0)
                    .hint_text(r"C:\FreeSpace\data\snapshots"),
            );
            if ui.button(t!("dashboard.trend").to_string()).clicked() {
                self.refresh();
            }
        });

        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        if let Some(message) = &self.message {
            ui.label(message);
        }

        let Some(dashboard) = &self.dashboard else {
            return None;
        };

        ui.add_space(8.0);
        ui.separator();
        let mut analyze = None;

        egui::Grid::new("dashboard_grid")
            .num_columns(7)
            .striped(true)
            .spacing([16.0, 6.0])
            .show(ui, |ui| {
                for heading in [
                    t!("dashboard.volume").to_string(),
                    t!("dashboard.filesystem").to_string(),
                    t!("dashboard.total").to_string(),
                    t!("dashboard.used").to_string(),
                    t!("dashboard.free").to_string(),
                    t!("dashboard.usage").to_string(),
                    String::new(),
                ] {
                    ui.strong(heading);
                }
                ui.end_row();

                for summary in &dashboard.volumes {
                    let volume = &summary.volume;
                    ui.label(if volume.label.is_empty() {
                        volume.mount_point.display().to_string()
                    } else {
                        format!("{} ({})", volume.mount_point.display(), volume.label)
                    });
                    ui.label(volume.file_system.as_str().to_uppercase());
                    ui.label(if volume.status == VolumeStatus::Available {
                        format_bytes(volume.total_bytes, units)
                    } else {
                        volume.status.as_str().to_owned()
                    });
                    ui.label(format_bytes(summary.used_bytes, units));
                    ui.label(format_bytes(volume.free_bytes, units));
                    ui.add(
                        egui::ProgressBar::new(summary.usage_ratio())
                            .text(format!("{}%", summary.used_percent)),
                    );
                    if summary.is_available()
                        && ui.button(t!("dashboard.analyze").to_string()).clicked()
                    {
                        analyze = Some(summary.mount_point().to_path_buf());
                    } else {
                        ui.label("");
                    }
                    ui.end_row();
                }
            });

        ui.add_space(12.0);
        ui.separator();
        ui.heading(t!("dashboard.trend").to_string());
        let mut any_trend = false;
        for summary in &dashboard.volumes {
            if summary.trend.is_empty() {
                continue;
            }
            any_trend = true;
            ui.label(format!(
                "{}: {} · {}",
                summary.volume.mount_point.display(),
                t!(
                    "dashboard.snapshot_count",
                    count = summary.trend.points.len()
                ),
                t!(
                    "dashboard.trend_growth",
                    size = signed(summary.trend.change_bytes(), units)
                )
            ));
            let values: Vec<f32> = summary
                .trend
                .points
                .iter()
                .map(|point| {
                    if summary.volume.total_bytes == 0 {
                        0.0
                    } else {
                        #[allow(clippy::cast_precision_loss)]
                        let ratio = point.used_bytes as f32 / summary.volume.total_bytes as f32;
                        ratio.clamp(0.0, 1.0)
                    }
                })
                .collect();
            let (response, painter) =
                ui.allocate_painter(egui::vec2(ui.available_width(), 48.0), egui::Sense::hover());
            draw_trend(
                &painter,
                response.rect,
                &values,
                ui.visuals().selection.bg_fill,
            );
        }
        if !any_trend {
            ui.label(t!("dashboard.trend_empty").to_string());
        }

        analyze
    }

    fn load_trends(&self) -> HashMap<Uuid, Vec<SnapshotPoint>> {
        let mut trends: HashMap<Uuid, Vec<SnapshotPoint>> = HashMap::new();
        let directory = PathBuf::from(self.snapshots_dir.trim());
        if self.snapshots_dir.trim().is_empty() || !directory.is_dir() {
            return trends;
        }

        let Ok(entries) = std::fs::read_dir(directory) else {
            return trends;
        };
        for entry in entries.flatten().take(1_000) {
            let path = entry.path();
            if !path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("fssnap"))
            {
                continue;
            }
            let Ok(payload) = SnapshotPayload::read(&path) else {
                continue;
            };
            let Some(taken_at) = Timestamp::parse_iso8601(&payload.created_at) else {
                continue;
            };
            let mount_point = Path::new(&payload.volume.mount_point);
            let used_bytes = payload
                .volume
                .total_bytes
                .saturating_sub(payload.volume.free_bytes);
            trends
                .entry(Volume::id_for_mount_point(mount_point))
                .or_default()
                .push(SnapshotPoint::new(
                    taken_at,
                    used_bytes,
                    payload.volume.free_bytes,
                ));
        }

        trends
    }
}

fn signed(bytes: i64, units: Units) -> String {
    if bytes < 0 {
        format!("-{}", format_bytes(bytes.unsigned_abs(), units))
    } else {
        format!(
            "+{}",
            format_bytes(u64::try_from(bytes).unwrap_or(0), units)
        )
    }
}

fn draw_trend(painter: &egui::Painter, rect: egui::Rect, values: &[f32], colour: egui::Color32) {
    if values.len() < 2 {
        return;
    }
    #[allow(clippy::cast_precision_loss)]
    let step = rect.width() / (values.len() - 1) as f32;
    let points: Vec<egui::Pos2> = values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            #[allow(clippy::cast_precision_loss)]
            let x = rect.left() + index as f32 * step;
            let y = rect.bottom() - value.clamp(0.0, 1.0) * rect.height();

            egui::pos2(x, y)
        })
        .collect();
    for pair in points.windows(2) {
        painter.line_segment([pair[0], pair[1]], egui::Stroke::new(1.5, colour));
    }
}

#[cfg(test)]
mod tests {
    use super::signed;
    use freespace_core::size::Units;

    #[test]
    fn signs_trend_changes() {
        assert_eq!(signed(0, Units::Binary), "+0 B");
        assert!(signed(1_048_576, Units::Binary).starts_with('+'));
        assert!(signed(-1_048_576, Units::Binary).starts_with('-'));
        assert_eq!(signed(1_000, Units::Decimal), "+1 KB");
    }
}
