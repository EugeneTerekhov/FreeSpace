//! Attributes and NTFS permissions screen (FR-057…FR-060).

use std::path::PathBuf;

use eframe::egui;
use freespace_ops::{
    AclAction, AttributeChange, Store, acl::apply_acl, attributes::apply_attributes, store,
};
/// State of the permissions screen.
#[derive(Debug, Default)]
pub struct PermissionsState {
    paths_text: String,
    set: String,
    clear: String,
    owner: String,
    reset: bool,
    copy_from: String,
    confirmed: bool,
    message: Option<String>,
    errors: Vec<String>,
}

impl PermissionsState {
    /// Draws the screen.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.label(t!("permissions.intro").to_string());
        ui.add_space(8.0);

        ui.label(t!("permissions.objects").to_string());
        ui.add(
            egui::TextEdit::multiline(&mut self.paths_text)
                .desired_rows(4)
                .desired_width(f32::INFINITY)
                .hint_text(r"C:\data\file.bin"),
        );

        ui.separator();
        ui.heading(t!("permissions.attributes").to_string());
        ui.horizontal(|ui| {
            ui.label(t!("permissions.set").to_string());
            ui.text_edit_singleline(&mut self.set);
            ui.label(t!("permissions.clear").to_string());
            ui.text_edit_singleline(&mut self.clear);
            if ui
                .button(t!("permissions.apply_attrs").to_string())
                .clicked()
            {
                self.apply_attributes();
            }
        });
        ui.label(t!("permissions.attributes_hint").to_string());

        ui.separator();
        ui.heading(t!("permissions.acl").to_string());
        ui.horizontal(|ui| {
            ui.label(t!("permissions.owner").to_string());
            ui.text_edit_singleline(&mut self.owner);
            if ui
                .button(t!("permissions.apply_owner").to_string())
                .clicked()
            {
                self.apply_acl(AclAction::SetOwner(self.owner.trim().to_owned()));
            }
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.reset, t!("permissions.reset").to_string());
            if ui
                .button(t!("permissions.apply_reset").to_string())
                .clicked()
            {
                self.apply_acl(AclAction::Reset);
            }
        });
        ui.horizontal(|ui| {
            ui.label(t!("permissions.copy_from").to_string());
            ui.text_edit_singleline(&mut self.copy_from);
            if ui
                .button(t!("permissions.apply_copy").to_string())
                .clicked()
            {
                let source = PathBuf::from(self.copy_from.trim());
                self.apply_acl(AclAction::CopyFrom(source));
            }
        });

        ui.add_space(8.0);
        ui.checkbox(&mut self.confirmed, t!("permissions.confirm").to_string());

        if let Some(message) = &self.message {
            ui.label(message);
        }
        for error in &self.errors {
            ui.colored_label(ui.visuals().error_fg_color, error.clone());
        }
    }

    fn targets(&self) -> Vec<PathBuf> {
        self.paths_text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(PathBuf::from)
            .collect()
    }

    fn open_store() -> freespace_core::Result<Store> {
        let path = store::default_path()?;

        Store::open(&path)
    }

    fn apply_attributes(&mut self) {
        self.reset_messages();
        if !self.confirmed {
            self.errors
                .push(t!("permissions.confirm_required").to_string());
            return;
        }
        let result =
            AttributeChange::parse(Some(&self.set), Some(&self.clear)).and_then(|change| {
                let store = Self::open_store()?;
                apply_attributes(Some(&store), &self.targets(), change)
            });

        match result {
            Ok(report) => {
                self.message = Some(
                    t!(
                        "permissions.updated",
                        succeeded = report.succeeded(),
                        failed = report.failed()
                    )
                    .to_string(),
                );
                self.errors
                    .extend(report.items.into_iter().filter_map(|item| item.error));
            }
            Err(error) => self.errors.push(error.to_string()),
        }
    }

    fn apply_acl(&mut self, action: AclAction) {
        self.reset_messages();
        if !self.confirmed {
            self.errors
                .push(t!("permissions.confirm_required").to_string());
            return;
        }
        let result =
            Self::open_store().and_then(|store| apply_acl(Some(&store), &self.targets(), &action));

        match result {
            Ok(report) => {
                self.message = Some(
                    t!(
                        "permissions.updated",
                        succeeded = report.succeeded(),
                        failed = report.failed()
                    )
                    .to_string(),
                );
                self.errors
                    .extend(report.items.into_iter().filter_map(|item| item.error));
            }
            Err(error) => self.errors.push(error.to_string()),
        }
    }

    fn reset_messages(&mut self) {
        self.message = None;
        self.errors.clear();
    }

    /// Number of records errors in the last run; used by tests.
    #[cfg(test)]
    #[must_use]
    pub fn errors(&self) -> usize {
        self.errors.len()
    }
}

#[cfg(test)]
mod tests {
    use super::PermissionsState;

    #[test]
    fn starts_without_errors() {
        assert_eq!(PermissionsState::default().errors(), 0);
    }

    #[test]
    fn splits_targets_on_lines() {
        let state = PermissionsState {
            paths_text: "C:\\a.txt\n\n C:\\b.txt \n".to_owned(),
            ..PermissionsState::default()
        };

        assert_eq!(state.targets().len(), 2);
    }
}
