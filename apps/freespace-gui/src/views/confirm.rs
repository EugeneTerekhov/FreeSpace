//! Confirmation dialog for destructive operations (FR-042, FR-043).
//!
//! The dialog shows every affected object, the total size, the consequences of
//! each deletion mode and an explicit confirmation box. Nothing is executed from
//! this module: it only reports the user's decision.

use eframe::egui;
use freespace_core::size::{Units, format_bytes};
use freespace_ops::{Preview, confirm::delete_mode_consequence, operation::DeleteMode};
use freespace_platform::paths;

/// User decision produced by the dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirmation {
    /// The dialog is still open.
    Pending,
    /// The user confirmed the selected mode.
    Confirmed,
    /// The user closed the dialog.
    Cancelled,
}

/// State of the confirmation dialog.
#[derive(Debug)]
pub struct DeletionDialog {
    preview: Preview,
    mode: DeleteMode,
    confirmed: bool,
    extra_confirmed: bool,
}

impl DeletionDialog {
    /// Creates a dialog from a deletion preview.
    #[must_use]
    pub fn new(preview: Preview) -> Self {
        Self {
            preview,
            mode: DeleteMode::Quarantine,
            confirmed: false,
            extra_confirmed: false,
        }
    }

    /// Currently selected mode.
    #[must_use]
    pub const fn mode(&self) -> DeleteMode {
        self.mode
    }

    /// Returns `true` when the user may execute the operation.
    #[must_use]
    pub fn can_execute(&self) -> bool {
        self.confirmed
            && !self.preview.insufficient_space.is_some_and(|_| {
                matches!(self.mode, DeleteMode::RecycleBin | DeleteMode::Quarantine)
            })
            && (!self.preview.requires_extra_confirmation || self.extra_confirmed)
    }

    /// Draws the dialog and returns the user decision.
    pub fn ui(&mut self, ui: &mut egui::Ui, units: Units) -> Confirmation {
        ui.heading(t!("confirm.title").to_string());
        ui.label(
            t!(
                "confirm.summary",
                count = self.preview.total_count,
                size = format_bytes(self.preview.total_bytes, units)
            )
            .to_string(),
        );

        egui::ScrollArea::vertical()
            .max_height(240.0)
            .show(ui, |ui| {
                for item in &self.preview.items {
                    ui.label(format!(
                        "{:>12}  {}",
                        format_bytes(item.size, units),
                        paths::display(&item.path)
                    ));
                }
            });

        for warning in &self.preview.warnings {
            ui.colored_label(ui.visuals().warn_fg_color, warning.clone());
        }
        if let Some(shortage) = self.preview.insufficient_space {
            ui.colored_label(
                ui.visuals().error_fg_color,
                t!(
                    "confirm.insufficient_space",
                    required = format_bytes(shortage.required, units),
                    available = format_bytes(shortage.available, units)
                )
                .to_string(),
            );
        }

        ui.separator();
        ui.label(t!("confirm.mode").to_string());
        ui.horizontal_wrapped(|ui| {
            for option in &self.preview.delete_options {
                if ui
                    .selectable_label(self.mode == option.mode, option.mode.as_str())
                    .clicked()
                {
                    self.mode = option.mode;
                }
            }
        });
        ui.label(t!(delete_mode_consequence(self.mode)).to_string());

        if self.preview.requires_extra_confirmation {
            ui.checkbox(&mut self.extra_confirmed, t!("confirm.extra").to_string());
        }
        ui.checkbox(&mut self.confirmed, t!("confirm.checkbox").to_string());

        let mut decision = Confirmation::Pending;
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    self.can_execute(),
                    egui::Button::new(t!("confirm.apply").to_string()),
                )
                .clicked()
            {
                decision = Confirmation::Confirmed;
            }
            if ui.button(t!("common.cancel").to_string()).clicked() {
                decision = Confirmation::Cancelled;
            }
        });

        decision
    }

    /// Preview being confirmed.
    #[must_use]
    pub const fn preview(&self) -> &Preview {
        &self.preview
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use freespace_core::size::Units;
    use freespace_ops::{
        DeleteMode, Preview, PreviewItem, PreviewItemKind, SpaceShortage, confirm::preview_delete,
    };

    use super::{Confirmation, DeletionDialog};

    fn preview() -> Preview {
        preview_delete(&[PathBuf::from(r"C:\temp\a.txt")], Some(10)).expect("preview")
    }

    #[test]
    fn requires_the_confirmation_checkbox() {
        let mut dialog = DeletionDialog::new(preview());

        assert!(!dialog.can_execute());
        dialog.confirmed = true;
        assert!(dialog.can_execute());
        assert_eq!(dialog.mode(), DeleteMode::Quarantine);
        assert_eq!(Confirmation::Pending, Confirmation::Pending);
    }

    #[test]
    fn reports_insufficient_space_for_copying_modes() {
        let mut dialog = DeletionDialog::new(Preview {
            kind: freespace_ops::OperationKind::Delete,

            items: vec![PreviewItem {
                path: PathBuf::from(r"C:\temp\a.txt"),
                size: 100,
                kind: PreviewItemKind::File,
            }],
            total_bytes: 100,
            total_count: 1,
            delete_options: Vec::new(),
            requires_elevation: false,
            requires_extra_confirmation: false,
            insufficient_space: Some(SpaceShortage {
                required: 100,
                available: 10,
            }),
            warnings: Vec::new(),
        });
        dialog.confirmed = true;

        assert!(!dialog.can_execute());
        dialog.mode = DeleteMode::Permanent;
        assert!(dialog.can_execute());
        let _ = Units::Binary;
    }
}
