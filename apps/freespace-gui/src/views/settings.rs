//! Settings screen (US23, FR-091…FR-093).
//!
//! The screen edits the shared [`Settings`] value; the shell applies the theme
//! and language immediately and persists the file on request.

use eframe::egui;
use freespace_core::{
    i18n::Locale,
    settings::{DuplicateKeepRule, Settings, Theme},
    size::Units,
};

/// What the shell should do after drawing the screen.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SettingsAction {
    /// Nothing changed.
    #[default]
    None,
    /// A value changed; apply live effects.
    Changed,
    /// The user pressed *Save*.
    Save,
}

/// State of the settings screen.
#[derive(Debug, Default)]
pub struct SettingsState {
    saved: bool,
    error: Option<String>,
    settings_path: Option<String>,
}

impl SettingsState {
    /// Records the current settings file path for display.
    pub fn set_path(&mut self, path: Option<&std::path::Path>) {
        self.settings_path = path.map(|path| path.display().to_string());
    }

    /// Marks the settings as saved or failed.
    pub fn set_result(&mut self, result: Result<(), String>) {
        match result {
            Ok(()) => {
                self.saved = true;
                self.error = None;
            }
            Err(error) => {
                self.saved = false;
                self.error = Some(error);
            }
        }
    }

    /// Draws the screen and reports the requested action.
    pub fn ui(&mut self, ui: &mut egui::Ui, settings: &mut Settings) -> SettingsAction {
        let mut action = SettingsAction::None;

        egui::Grid::new("settings_grid")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label(t!("settings.language").to_string());
                ui.horizontal(|ui| {
                    let previous = settings.language;
                    for locale in Locale::ALL {
                        let label = match locale {
                            Locale::Ru => "Русский",
                            Locale::En => "English",
                        };
                        ui.selectable_value(&mut settings.language, locale, label);
                    }
                    if settings.language != previous {
                        action = SettingsAction::Changed;
                    }
                });
                ui.end_row();

                ui.label(t!("settings.theme").to_string());
                ui.horizontal(|ui| {
                    let previous = settings.theme;
                    for (theme, label) in [
                        (Theme::System, "System"),
                        (Theme::Light, "Light"),
                        (Theme::Dark, "Dark"),
                    ] {
                        ui.selectable_value(&mut settings.theme, theme, label);
                    }
                    if settings.theme != previous {
                        action = SettingsAction::Changed;
                    }
                });
                ui.end_row();

                ui.label(t!("settings.units").to_string());
                ui.horizontal(|ui| {
                    let previous = settings.units;
                    ui.selectable_value(&mut settings.units, Units::Binary, "binary (KiB)");
                    ui.selectable_value(&mut settings.units, Units::Decimal, "decimal (KB)");
                    if settings.units != previous {
                        action = SettingsAction::Changed;
                    }
                });
                ui.end_row();

                ui.label(t!("settings.duplicate_rule").to_string());
                ui.horizontal(|ui| {
                    let previous = settings.duplicate_keep_rule;
                    egui::ComboBox::from_id_salt("duplicate_rule")
                        .selected_text(settings.duplicate_keep_rule.as_str())
                        .show_ui(ui, |ui| {
                            for rule in [
                                DuplicateKeepRule::Oldest,
                                DuplicateKeepRule::Newest,
                                DuplicateKeepRule::ShortestPath,
                                DuplicateKeepRule::OriginalDir,
                            ] {
                                ui.selectable_value(
                                    &mut settings.duplicate_keep_rule,
                                    rule,
                                    rule.as_str(),
                                );
                            }
                        });
                    if settings.duplicate_keep_rule != previous {
                        action = SettingsAction::Changed;
                    }
                });
                ui.end_row();

                ui.label(t!("settings.exclusions").to_string());
                let mut exclusions = settings.exclusions.join("; ");
                if ui
                    .add(
                        egui::TextEdit::multiline(&mut exclusions)
                            .desired_rows(2)
                            .desired_width(360.0),
                    )
                    .changed()
                {
                    settings.exclusions = exclusions
                        .split([';', '|'])
                        .map(str::trim)
                        .filter(|entry| !entry.is_empty())
                        .map(str::to_owned)
                        .collect();
                    action = SettingsAction::Changed;
                }
                ui.end_row();

                ui.label(t!("settings.outdated_days").to_string());
                if ui
                    .add(
                        egui::DragValue::new(&mut settings.outdated_threshold_days).range(0..=3650),
                    )
                    .changed()
                {
                    action = SettingsAction::Changed;
                }
                ui.end_row();

                ui.label(t!("settings.quarantine_days").to_string());
                if ui
                    .add(
                        egui::DragValue::new(&mut settings.quarantine_retention_days)
                            .range(0..=3650),
                    )
                    .changed()
                {
                    action = SettingsAction::Changed;
                }
                ui.end_row();

                ui.label(t!("settings.monitor_threshold").to_string());
                if ui
                    .add(
                        egui::DragValue::new(&mut settings.monitor_threshold.threshold_bytes)
                            .range(0..=u64::MAX)
                            .speed(1_048_576.0),
                    )
                    .changed()
                {
                    action = SettingsAction::Changed;
                }
                ui.end_row();

                ui.label(t!("settings.monitor_interval").to_string());
                if ui
                    .add(
                        egui::DragValue::new(&mut settings.monitor_threshold.interval_secs)
                            .range(1..=86_400),
                    )
                    .changed()
                {
                    action = SettingsAction::Changed;
                }
                ui.end_row();

                ui.label(t!("settings.portable").to_string());
                if ui.checkbox(&mut settings.portable_mode, "").changed() {
                    action = SettingsAction::Changed;
                }
                ui.end_row();

                ui.label(t!("settings.confirm").to_string());
                if ui
                    .checkbox(&mut settings.confirm_destructive_actions, "")
                    .changed()
                {
                    action = SettingsAction::Changed;
                }
                ui.end_row();
            });

        if let Some(path) = &self.settings_path {
            ui.add_space(8.0);
            ui.label(format!("{}: {path}", t!("settings.path")));
        }
        ui.label(t!("settings.restart_hint").to_string());

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui.button(t!("settings.save").to_string()).clicked() {
                action = SettingsAction::Save;
            }
            if self.saved {
                ui.label(t!("settings.saved").to_string());
            }
            if let Some(error) = &self.error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
        });

        action
    }
}
