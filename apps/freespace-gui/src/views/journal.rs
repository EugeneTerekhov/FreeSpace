//! Operation journal with undo and quarantine restore (FR-044…FR-046).

use eframe::egui;
use freespace_core::{
    Uuid,
    size::{Units, format_bytes},
};
use freespace_ops::{JournalEntry, Store, quarantine::Quarantine, store, undo::undo_operation};

/// State of the journal screen.
#[derive(Debug, Default)]
pub struct JournalState {
    entries: Vec<JournalEntry>,
    message: Option<String>,
    error: Option<String>,
    loaded: bool,
}

impl JournalState {
    /// Draws the screen.
    pub fn ui(&mut self, ui: &mut egui::Ui, portable: bool, units: Units) {
        ui.label(t!("journal.intro").to_string());
        ui.horizontal(|ui| {
            if ui.button(t!("journal.refresh").to_string()).clicked() || !self.loaded {
                self.reload();
            }
        });

        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        if let Some(message) = &self.message {
            ui.label(message);
        }

        ui.add_space(8.0);
        if self.entries.is_empty() {
            ui.label(t!("journal.empty").to_string());
            return;
        }

        let mut undo_id = None;
        for entry in &self.entries {
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.label(entry.timestamp.format_utc());
                ui.label(&entry.user);
                ui.label(format!(
                    "{}: {} objects, {}",
                    entry.result.as_str(),
                    entry.affected_count,
                    format_bytes(entry.affected_bytes, units)
                ));
                ui.label(&entry.summary);
            });
            ui.horizontal(|ui| {
                ui.label(entry.operation_id.to_string());
                if entry.undo_available && ui.button(t!("journal.undo").to_string()).clicked() {
                    undo_id = Some(entry.operation_id);
                }
            });
        }
        if let Some(operation_id) = undo_id {
            self.undo(operation_id, portable);
        }
    }

    fn reload(&mut self) {
        self.error = None;
        self.loaded = true;
        match open_store().and_then(|store| store.journal_list(200)) {
            Ok(entries) => self.entries = entries,
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn undo(&mut self, operation_id: Uuid, portable: bool) {
        self.error = None;
        self.message = None;
        let result = open_store().and_then(|store| {
            let root = Quarantine::default_root(portable)?;
            let quarantine = Quarantine::new(root)?;
            undo_operation(&store, &quarantine, operation_id)
        });

        match result {
            Ok(outcome) => {
                self.message = Some(
                    t!(
                        "journal.restored",
                        restored = outcome.restored,
                        failed = outcome.failed
                    )
                    .to_string(),
                );
                for error in outcome.errors {
                    self.error = Some(error);
                }
                self.reload();
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    /// Number of loaded entries; used by tests.
    #[cfg(test)]
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

fn open_store() -> freespace_core::Result<Store> {
    let path = store::default_path()?;

    Store::open(&path)
}

#[cfg(test)]
mod tests {
    use super::JournalState;

    #[test]
    fn starts_without_entries() {
        assert_eq!(JournalState::default().len(), 0);
    }
}
