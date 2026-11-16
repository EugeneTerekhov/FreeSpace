//! Elevation screen (US21).
//!
//! Explains why protected system areas need administrator rights, shows the
//! current mode and probes the well-known protected directories. The elevated
//! instance is started only after the user presses the button.

use std::ffi::OsString;

use eframe::egui;
use freespace_platform::elevation::{self, ElevationResult};
use freespace_scan::access::{self, AccessPolicy, Probe};

use freespace_core::size::Units;

/// State of the elevation screen.
#[derive(Debug, Default)]
pub struct ElevationState {
    reason: String,
    message: Option<String>,
    error: Option<String>,
    areas: Vec<(String, Probe)>,
    loaded: bool,
}

impl ElevationState {
    /// Refreshes the elevation state and the protected-directory probes.
    pub fn refresh(&mut self) {
        let policy = AccessPolicy::current();
        let mut areas = Vec::new();
        if let Ok(volumes) = freespace_platform::volumes::list_volumes() {
            for volume in volumes
                .iter()
                .filter(|volume| volume.status == freespace_core::model::VolumeStatus::Available)
            {
                for area in access::protected_areas(&volume.mount_point) {
                    areas.push((area.path.display().to_string(), area.probe));
                }
            }
        }
        self.areas = areas;
        self.loaded = true;
        if policy.elevated {
            self.message = Some(t!("elevation.elevated").to_string());
        }
    }

    /// Draws the screen.
    pub fn ui(&mut self, ui: &mut egui::Ui, _units: Units) {
        if !self.loaded {
            self.refresh();
        }

        let elevated = elevation::is_elevated();
        ui.label(t!("elevation.intro").to_string());
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.strong(t!("elevation.status").to_string());
            ui.label(if elevated {
                t!("elevation.elevated").to_string()
            } else {
                t!("elevation.limited").to_string()
            });
        });

        if !elevated {
            ui.add_space(8.0);
            ui.label(t!("elevation.reason").to_string());
            ui.add(
                egui::TextEdit::multiline(&mut self.reason)
                    .desired_rows(3)
                    .desired_width(ui.available_width()),
            );
            if ui.button(t!("elevation.restart").to_string()).clicked() {
                self.request_elevation();
            }
        }

        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        if let Some(message) = &self.message {
            ui.label(message);
        }

        ui.add_space(12.0);
        ui.separator();
        ui.heading(t!("elevation.protected").to_string());
        ui.label(t!("elevation.protected_hint").to_string());

        egui::Grid::new("protected_areas")
            .num_columns(2)
            .striped(true)
            .spacing([16.0, 4.0])
            .show(ui, |ui| {
                for (path, probe) in &self.areas {
                    ui.label(path);
                    ui.label(match probe {
                        Probe::Available => t!("elevation.available").to_string(),
                        Probe::Denied => t!("elevation.denied").to_string(),
                        Probe::Missing => t!("elevation.not_found").to_string(),
                        Probe::Failed(reason) => reason.clone(),
                    });
                    ui.end_row();
                }
            });

        if ui.button(t!("dashboard.refresh").to_string()).clicked() {
            self.refresh();
        }
    }

    fn request_elevation(&mut self) {
        let Some(executable) = elevation::current_executable() else {
            self.error = Some("cannot resolve the FreeSpace executable".to_owned());
            return;
        };
        let arguments: Vec<OsString> = elevation::relaunch_arguments();

        match elevation::relaunch(&executable, &arguments) {
            Ok(ElevationResult::Launched) => {
                self.message = Some("An elevated instance is starting.".to_owned());
                self.error = None;
            }
            Ok(ElevationResult::Declined) => {
                self.message = Some(t!("elevation.declined").to_string());
                self.error = None;
            }
            Err(error) => {
                self.error = Some(error.to_string());
            }
        }
    }
}
