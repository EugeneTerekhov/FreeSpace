//! Undo and restore for journalled operations (FR-044, FR-046).
//!
//! Undo is deliberately conservative: it only restores objects that are still in
//! quarantine and refuses to overwrite an object that exists again. Renames are
//! reversed only when the destination still exists and the original path is
//! free.

use std::{
    fs,
    path::{Path, PathBuf},
};

use freespace_core::{Error, Result, Uuid};
use freespace_platform::paths;

use crate::{quarantine::Quarantine, store::Store};

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

/// Restores everything that the journal entry recorded as undoable.
///
/// # Errors
/// Returns [`Error::NotFound`] when the journal entry or a quarantined object is
/// gone, [`Error::Conflict`] when undo is unavailable, and the first error when
/// nothing could be restored.
pub fn undo_operation(
    store: &Store,
    quarantine: &Quarantine,
    operation_id: Uuid,
) -> Result<UndoOutcome> {
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

    if let Some(ids) = entry
        .details
        .get("quarantine_ids")
        .and_then(|ids| ids.as_array())
    {
        for id in ids {
            let Some(id) = id.as_str().and_then(|value| Uuid::parse_str(value).ok()) else {
                outcome.failed = outcome.failed.saturating_add(1);
                outcome
                    .errors
                    .push("malformed quarantine identifier".to_owned());
                continue;
            };
            let Some(item) = store.quarantine_get(id)? else {
                outcome.failed = outcome.failed.saturating_add(1);
                outcome
                    .errors
                    .push(format!("quarantine item `{id}` was not found"));
                continue;
            };
            match quarantine.restore(store, &item) {
                Ok(()) => outcome.restored = outcome.restored.saturating_add(1),
                Err(error) => {
                    outcome.failed = outcome.failed.saturating_add(1);
                    outcome
                        .errors
                        .push(format!("{}: {error}", paths::display(&item.original_path)));
                }
            }
        }
    }

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
    let from_verbatim = paths::to_verbatim(from)?;
    let to_verbatim = paths::to_verbatim(to)?;

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

/// Returns the original paths of the quarantine objects referenced by an entry.
///
/// # Errors
/// Returns an error when the quarantine index cannot be read.
pub fn affected_paths(store: &Store, entry: &crate::JournalEntry) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    if let Some(ids) = entry
        .details
        .get("quarantine_ids")
        .and_then(|ids| ids.as_array())
    {
        for id in ids {
            if let Some(item) = id
                .as_str()
                .and_then(|value| Uuid::parse_str(value).ok())
                .map(|id| store.quarantine_get(id))
                .transpose()?
                .flatten()
            {
                paths.push(item.original_path);
            }
        }
    }

    Ok(paths)
}

#[cfg(test)]
mod tests {
    use freespace_core::ErrorCode;

    use super::undo_operation;
    use crate::{
        delete::{DeleteOptions, delete_paths},
        operation::DeleteMode,
        quarantine::Quarantine,
        store::Store,
    };

    #[test]
    fn restores_quarantined_objects_and_clears_undo() {
        let workspace = tempfile::tempdir().expect("workspace");
        let root = tempfile::tempdir().expect("quarantine");
        let store = Store::open_in_memory().expect("store");
        let path = workspace.path().join("restore.txt");
        std::fs::write(&path, b"payload").expect("write");
        let quarantine = Quarantine::new(root.path().join("store")).expect("quarantine");

        let outcome = delete_paths(
            &store,
            Some(&quarantine),
            std::slice::from_ref(&path),
            &DeleteOptions::new(DeleteMode::Quarantine),
            true,
        )
        .expect("delete");
        let operation_id = store
            .journal_list(1)
            .expect("journal")
            .first()
            .expect("entry")
            .operation_id;

        let undone = undo_operation(&store, &quarantine, operation_id).expect("undo");

        assert_eq!(undone.restored, 1);
        assert!(undone.is_complete());
        assert_eq!(std::fs::read(&path).expect("read"), b"payload");
        assert!(!store.journal_list(1).expect("journal")[0].undo_available);
        assert!(outcome.undoable);

        let error = undo_operation(&store, &quarantine, operation_id).expect_err("second undo");
        assert_eq!(error.code(), ErrorCode::Conflict);
    }

    #[test]
    fn refuses_unknown_operations() {
        let root = tempfile::tempdir().expect("quarantine");
        let store = Store::open_in_memory().expect("store");
        let quarantine = Quarantine::new(root.path().join("store")).expect("quarantine");

        let error = undo_operation(&store, &quarantine, freespace_core::Uuid::new_v4())
            .expect_err("unknown");

        assert_eq!(error.code(), ErrorCode::NotFound);
    }
}
