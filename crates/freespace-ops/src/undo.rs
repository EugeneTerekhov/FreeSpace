//! Undo for journalled operations (FR-044, FR-046).
//!
//! Undo is deliberately conservative: only renames are reversed, and only when
//! the destination still exists and the original path is free. Deleting to the
//! Recycle Bin is restored through the shell, not here.

use std::{fs, path::Path};

use freespace_core::{Error, Result, Uuid};
use freespace_platform::paths;

use crate::{path_compat, store::Store};

/// Result of an undo attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoOutcome {
    /// Objects restored successfully.
    pub restored: u32,
    /// Objects that could not be restored.
    pub failed: u32,
    /// Human-readable failures.
    pub errors: Vec<String>,
}

impl UndoOutcome {
    /// Returns `true` when every object was restored.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.failed == 0
    }
}

/// Reverses the renames that the journal entry recorded as undoable.
///
/// # Errors
/// Returns [`Error::NotFound`] when the journal entry is gone,
/// [`Error::Conflict`] when undo is unavailable, and the first error when
/// nothing could be restored.
pub fn undo_operation(store: &Store, operation_id: Uuid) -> Result<UndoOutcome> {
    let entry = store
        .journal_by_operation(operation_id)?
        .ok_or_else(|| Error::NotFound(format!("journal entry for `{operation_id}`")))?;
    if !entry.undo_available {
        return Err(Error::Conflict(format!(
            "operation `{operation_id}` cannot be undone"
        )));
    }

    let mut outcome = UndoOutcome {
        restored: 0,
        failed: 0,
        errors: Vec::new(),
    };

    if let Some(renames) = entry
        .details
        .get("renames")
        .and_then(|items| items.as_array())
    {
        for item in renames {
            let Some(from) = item.get("from").and_then(|value| value.as_str()) else {
                outcome.failed = outcome.failed.saturating_add(1);
                outcome.errors.push("malformed rename record".to_owned());
                continue;
            };
            let Some(to) = item.get("to").and_then(|value| value.as_str()) else {
                outcome.failed = outcome.failed.saturating_add(1);
                outcome.errors.push("malformed rename record".to_owned());
                continue;
            };
            match reverse_rename(Path::new(to), Path::new(from)) {
                Ok(()) => outcome.restored = outcome.restored.saturating_add(1),
                Err(error) => {
                    outcome.failed = outcome.failed.saturating_add(1);
                    outcome.errors.push(format!("{to}: {error}"));
                }
            }
        }
    }

    if outcome.restored > 0 {
        store.journal_set_undo_available(entry.id, false)?;
    }
    if outcome.restored == 0 && outcome.failed > 0 {
        return Err(Error::Operation(
            outcome
                .errors
                .first()
                .cloned()
                .unwrap_or_else(|| "nothing could be restored".to_owned()),
        ));
    }

    Ok(outcome)
}

fn reverse_rename(from: &Path, to: &Path) -> Result<()> {
    let from_verbatim = path_compat::to_verbatim(from)?;
    let to_verbatim = path_compat::to_verbatim(to)?;

    if !from_verbatim.exists() {
        return Err(Error::NotFound(format!(
            "`{}` no longer exists",
            paths::display(from)
        )));
    }
    if to_verbatim.exists() {
        return Err(Error::Conflict(format!(
            "`{}` already exists",
            paths::display(to)
        )));
    }
    if let Some(parent) = to_verbatim.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(from_verbatim, to_verbatim)?;

    Ok(())
}

/// Lists journals that can currently be undone, newest first.
///
/// # Errors
/// Returns an error when the journal cannot be read.
pub fn undoable_entries(store: &Store, limit: u32) -> Result<Vec<crate::JournalEntry>> {
    let mut entries = store.journal_list(limit)?;
    entries.retain(|entry| entry.undo_available);

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use freespace_core::{ErrorCode, Timestamp, Uuid};

    use super::undo_operation;
    use crate::{
        operation::{JournalEntry, JournalResult},
        store::Store,
    };

    fn renamed_entry(from: &std::path::Path, to: &std::path::Path) -> JournalEntry {
        JournalEntry {
            id: 0,
            operation_id: Uuid::new_v4(),
            timestamp: Timestamp::from_millis(1_700_000_000_000),
            user: "tester".to_owned(),
            summary: "rename: 1 succeeded".to_owned(),
            affected_count: 1,
            affected_bytes: 0,
            result: JournalResult::Success,
            undo_available: true,
            details: serde_json::json!({
                "renames": [{
                    "from": from.to_string_lossy(),
                    "to": to.to_string_lossy(),
                }],
            }),
        }
    }

    #[test]
    fn reverses_recorded_renames_and_clears_undo() {
        let workspace = tempfile::tempdir().expect("workspace");
        let store = Store::open_in_memory().expect("store");
        let original = workspace.path().join("old.txt");
        let renamed = workspace.path().join("new.txt");
        std::fs::write(&original, b"payload").expect("write");
        std::fs::rename(&original, &renamed).expect("rename");

        let entry = renamed_entry(&original, &renamed);
        store.journal_append(&entry).expect("append");
        let operation_id = entry.operation_id;

        let undone = undo_operation(&store, operation_id).expect("undo");

        assert_eq!(undone.restored, 1);
        assert!(undone.is_complete());
        assert!(original.exists());
        assert!(!renamed.exists());
        assert_eq!(std::fs::read(&original).expect("read"), b"payload");
        assert!(!store.journal_list(1).expect("journal")[0].undo_available);

        let error = undo_operation(&store, operation_id).expect_err("second undo");
        assert_eq!(error.code(), ErrorCode::Conflict);
    }

    #[test]
    fn refuses_unknown_operations() {
        let store = Store::open_in_memory().expect("store");

        let error = undo_operation(&store, Uuid::new_v4()).expect_err("unknown");

        assert_eq!(error.code(), ErrorCode::NotFound);
    }
}
