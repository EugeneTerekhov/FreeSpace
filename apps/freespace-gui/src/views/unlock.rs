//! Unlock screen: list holders, release locks and schedule reboot deletion.

use std::path::PathBuf;

use eframe::egui;
use freespace_ops::{
    Store, UnlockMode,
    unlock::{self, UnlockOutcome, UnlockPlan},
};

/// State of the unlock page.
#[derive(Debug, Default)]
pub struct UnlockState {
    /// Path being inspected.
    pub path: String,
    plan: Option<UnlockPlan>,
    outcome: Option<UnlockOutcome>,
    error: Option<String>,
    confirm_force: bool,
    confirm_reboot: bool,
}

/// Deferred action so that the plan can be rendered without borrowing `self`.
#[derive(Debug, Clone, Copy)]
enum Action {
    Soft,
    Force,
    Reboot,
}

impl UnlockState {
    /// Draws the page.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.label(t!("unlock.intro").to_string());
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label(t!("unlock.path").to_string());
            ui.add(egui::TextEdit::singleline(&mut self.path).desired_width(520.0));
            if ui.button(t!("unlock.inspect").to_string()).clicked() {
                self.inspect();
            }
        });

        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }

        let plan = self.plan.clone();
        if let Some(plan) = plan {
            ui.add_space(12.0);
            ui.label(if plan.available {
                t!("unlock.available").to_string()
            } else {
                t!("unlock.locked").to_string()
            });
            if plan.requires_reboot {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    t!("unlock.needs_reboot").to_string(),
                );
            }

            if plan.locks.is_empty() {
                ui.label(t!("unlock.no_locks").to_string());
            } else {
                egui::Grid::new("unlock_locks")
                    .num_columns(5)
                    .striped(true)
                    .show(ui, |ui| {
                        ui.strong(t!("unlock.pid").to_string());
                        ui.strong(t!("unlock.process").to_string());
                        ui.strong(t!("unlock.type").to_string());
                        ui.strong(t!("unlock.service").to_string());
                        ui.strong(t!("unlock.state").to_string());
                        ui.end_row();

                        for lock in &plan.locks {
                            ui.label(lock.process_id.to_string());
                            ui.label(&lock.process_name);
                            ui.label(lock.application_type.as_str());
                            ui.label(&lock.service_name);
                            if lock.is_critical() {
                                ui.colored_label(
                                    ui.visuals().warn_fg_color,
                                    t!("unlock.critical").to_string(),
                                );
                            } else {
                                ui.label("");
                            }
                            ui.end_row();
                        }
                    });
            }

            ui.add_space(12.0);
            let mut action = None;
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        plan.can_soft_close,
                        egui::Button::new(t!("unlock.soft").to_string()),
                    )
                    .clicked()
                {
                    action = Some(Action::Soft);
                }

                ui.checkbox(
                    &mut self.confirm_force,
                    t!("unlock.confirm_force").to_string(),
                );
                let force_available = plan.can_force_close && self.confirm_force;
                if ui
                    .add_enabled(
                        force_available,
                        egui::Button::new(t!("unlock.force").to_string()),
                    )
                    .clicked()
                {
                    action = Some(Action::Force);
                }
            });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.checkbox(
                    &mut self.confirm_reboot,
                    t!("unlock.confirm_reboot").to_string(),
                );
                if ui
                    .add_enabled(
                        self.confirm_reboot,
                        egui::Button::new(t!("unlock.reboot").to_string()),
                    )
                    .clicked()
                {
                    action = Some(Action::Reboot);
                }
            });

            match action {
                Some(Action::Soft) => self.release(UnlockMode::Soft),
                Some(Action::Force) => {
                    self.confirm_force = false;
                    self.release(UnlockMode::Force);
                }
                Some(Action::Reboot) => {
                    self.confirm_reboot = false;
                    self.schedule_reboot();
                }
                None => {}
            }
        }

        if let Some(outcome) = &self.outcome {
            ui.add_space(12.0);
            ui.separator();
            ui.label(format!(
                "{}: {} · {}: {}",
                t!("unlock.result"),
                outcome.mode.as_str(),
                t!("unlock.released"),
                outcome.released
            ));
            if outcome.scheduled_for_reboot {
                ui.label(t!("unlock.scheduled").to_string());
            }
            for warning in &outcome.warnings {
                ui.colored_label(ui.visuals().warn_fg_color, warning);
            }
        }
    }

    fn inspect(&mut self) {
        self.error = None;
        self.outcome = None;
        let path = PathBuf::from(self.path.trim());
        match unlock::plan(&path) {
            Ok(plan) => self.plan = Some(plan),
            Err(error) => {
                self.plan = None;
                self.error = Some(error.to_string());
            }
        }
    }

    fn release(&mut self, mode: UnlockMode) {
        self.error = None;
        let path = PathBuf::from(self.path.trim());
        match unlock::release(&path, mode, true) {
            Ok(outcome) => {
                self.record(&outcome);
                self.outcome = Some(outcome);
                self.inspect_after();
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn schedule_reboot(&mut self) {
        self.error = None;
        let path = PathBuf::from(self.path.trim());
        match unlock::delete_on_reboot(&path, true) {
            Ok(outcome) => {
                self.record(&outcome);
                self.outcome = Some(outcome);
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn inspect_after(&mut self) {
        let path = PathBuf::from(self.path.trim());
        if let Ok(plan) = unlock::plan(&path) {
            self.plan = Some(plan);
        }
    }

    fn record(&self, outcome: &UnlockOutcome) {
        let Ok(path) = freespace_ops::store::default_path() else {
            return;
        };
        let Ok(store) = Store::open(&path) else {
            return;
        };
        let user = std::env::var("USERNAME").unwrap_or_else(|_| "unknown".to_owned());
        let _ = unlock::record(&store, outcome, &user);
    }
}

#[cfg(test)]
mod tests {
    use super::UnlockState;

    #[test]
    fn starts_empty_and_accepts_a_path() {
        let state = UnlockState {
            path: r"C:\temp\locked.txt".to_owned(),
            ..UnlockState::default()
        };

        assert_eq!(state.path, r"C:\temp\locked.txt");
        assert!(state.plan.is_none());
        assert!(!state.confirm_force);
        assert!(!state.confirm_reboot);
    }
}
