//! Deletion through the Recycle Bin, the FreeSpace quarantine or permanently.
//!
//! Every mode goes through the same preview, free-space check, confirmation and
//! journal path (FR-042…FR-046). Quarantine deletions stay undoable; permanent
//! deletion is only reachable when the caller explicitly asks for it.

use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

use freespace_core::{Error, Result, Uuid};
use freespace_platform::{paths, recycle};

use crate::{
    confirm::{self, preview_delete, require_confirmation},
    operation::{
        DeleteMode, JournalEntry, Operation, OperationKind, OperationStatus, OperationTarget,
        ResultSummary, TargetStatus,
    },
    quarantine::Quarantine,
    store::Store,
};

/// Windows `ERROR_ACCESS_DENIED`.
const ERROR_ACCESS_DENIED: i32 = 5;

/// Parameters of a deletion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteOptions {
    /// Selected deletion mode.
    pub mode: DeleteMode,
    /// Short reason recorded in the quarantine index and the journal.
    pub reason: String,
    /// Quarantine retention in days.
    pub retention_days: u32,
    /// Free bytes on the target volume, when known.
    pub available_bytes: Option<u64>,
    /// Whether the caller confirmed the extra warning for protected paths.
    pub allow_protected: bool,
}

impl DeleteOptions {
    /// Creates options with default retention and reason.
    #[must_use]
    pub fn new(mode: DeleteMode) -> Self {
        Self {
            mode,
            reason: "cleanup".to_owned(),
            retention_days: 30,
            available_bytes: None,
            allow_protected: false,
        }
    }
}

/// Result for one deleted object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletedItem {
    /// Path that was processed.
    pub path: PathBuf,
    /// Size known before deletion.
    pub size: u64,
    /// Quarantine identifier, when the object went to quarantine.
    pub quarantine_id: Option<Uuid>,
    /// Error message, when the object could not be removed.
    pub error: Option<String>,
}

/// Aggregate result of a deletion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteOutcome {
    /// Identifier of the executed operation.
    pub operation_id: Uuid,
    /// Terminal state of the operation.
    pub status: OperationStatus,
    /// Per-object results.
    pub items: Vec<DeletedItem>,
    /// Summary of the underlying operation.
    pub summary: ResultSummary,
    /// Journal identifier, when the run was journalled.
    pub journal_id: Option<u64>,
    /// Number of objects that can be restored.
    pub undoable: bool,
}

impl DeleteOutcome {
    /// Returns the first failure, when any object failed.
    #[must_use]
    pub fn first_error(&self) -> Option<&str> {
        self.items.iter().find_map(|item| item.error.as_deref())
    }
}

/// Deletes `targets` using the selected mode.
///
/// # Errors
/// Returns [`Error::ConfirmationRequired`] without confirmation,
/// [`Error::InsufficientSpace`] when a copying mode cannot run,
/// [`Error::ProtectedPath`] for a protected path without the extra
/// confirmation, [`Error::InvalidArgument`] for a missing quarantine, and the
/// first error only when every target failed.
pub fn delete_paths(
    store: &Store,
    quarantine: Option<&Quarantine>,
    targets: &[PathBuf],
    options: &DeleteOptions,
    confirmed: bool,
) -> Result<DeleteOutcome> {
    delete_paths_cancellable(
        store,
        quarantine,
        targets,
        options,
        confirmed,
        Uuid::new_v4(),
        None,
        &mut |_, _, _| {},
    )
}

/// Deletes `targets`, stopping between objects when `cancel` is set.
///
/// The observer receives `(processed, total, path)` after every target, before
/// the next one starts. Cancellation is cooperative: the object in flight is
/// finished, the remaining targets stay pending and the operation ends as
/// `cancelled` with the actual partial result journalled.
///
/// # Errors
/// Returns the same errors as [`delete_paths`].
#[allow(clippy::too_many_arguments)]
pub fn delete_paths_cancellable(
    store: &Store,
    quarantine: Option<&Quarantine>,
    targets: &[PathBuf],
    options: &DeleteOptions,
    confirmed: bool,
    operation_id: Uuid,
    cancel: Option<&AtomicBool>,
    observer: &mut dyn FnMut(usize, usize, &Path),
) -> Result<DeleteOutcome> {
    require_confirmation(confirmed)?;
    if targets.is_empty() {
        return Err(Error::InvalidArgument(
            "a deletion needs at least one object".to_owned(),
        ));
    }
    if options.mode == DeleteMode::Quarantine && quarantine.is_none() {
        return Err(Error::InvalidArgument(
            "quarantine deletion requires a quarantine location".to_owned(),
        ));
    }

    let preview = preview_delete(targets, options.available_bytes)?;
    preview.ensure_space_for(options.mode)?;

    if !options.allow_protected {
        if let Some(protected) = targets.iter().find(|path| confirm::is_protected_path(path)) {
            return Err(Error::ProtectedPath(protected.clone()));
        }
    }

    let sizes: HashMap<PathBuf, u64> = preview
        .items
        .iter()
        .map(|item| (item.path.clone(), item.size))
        .collect();
    let operation_targets: Vec<OperationTarget> = targets
        .iter()
        .map(|path| OperationTarget::new(path.clone(), sizes.get(path).copied().unwrap_or(0)))
        .collect();
    let mut operation = Operation::with_id(OperationKind::Delete, operation_targets, operation_id)
        .with_delete_mode(options.mode)?;
    operation.requires_elevation = options.allow_protected;
    operation.confirm()?;
    operation.begin()?;

    let mut items = Vec::with_capacity(targets.len());
    let mut quarantine_ids = Vec::new();
    let mut first_error: Option<Error> = None;
    let mut cancelled = false;

    for (index, path) in targets.iter().enumerate() {
        if cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            cancelled = true;
            break;
        }
        let size = sizes.get(path).copied().unwrap_or(0);
        let result = delete_one(
            store,
            quarantine,
            path,
            options.mode,
            &options.reason,
            options.retention_days,
        );
        match result {
            Ok(quarantine_id) => {
                if let Some(id) = quarantine_id {
                    quarantine_ids.push(id.to_string());
                }
                operation.targets[index].status = TargetStatus::Succeeded;
                items.push(DeletedItem {
                    path: path.clone(),
                    size,
                    quarantine_id,
                    error: None,
                });
            }
            Err(error) => {
                operation.targets[index].status = TargetStatus::Failed;
                operation.targets[index].error = Some(error.to_string());
                if first_error.is_none() {
                    first_error = Some(error);
                }
                items.push(DeletedItem {
                    path: path.clone(),
                    size,
                    quarantine_id: None,
                    error: Some(operation.targets[index].error.clone().unwrap_or_default()),
                });
            }
        }
        observer(index + 1, targets.len(), path);
    }

    let succeeded = items.iter().filter(|item| item.error.is_none()).count();
    let status = if cancelled {
        OperationStatus::Cancelled
    } else if succeeded == items.len() {
        OperationStatus::Completed
    } else if succeeded == 0 {
        OperationStatus::Failed
    } else {
        OperationStatus::PartiallyCompleted
    };
    operation.finish(status)?;

    let mut entry = JournalEntry::from_operation(&operation, current_user());
    entry.undo_available = options.mode != DeleteMode::Permanent && succeeded > 0;
    if entry.details.is_null() {
        entry.details = serde_json::json!({});
    }
    if let Some(details) = entry.details.as_object_mut() {
        details.insert(
            "quarantine_ids".to_owned(),
            serde_json::Value::Array(
                quarantine_ids
                    .iter()
                    .map(|id| serde_json::Value::String(id.clone()))
                    .collect(),
            ),
        );
        details.insert(
            "reason".to_owned(),
            serde_json::Value::String(options.reason.clone()),
        );
        details.insert(
            "status".to_owned(),
            serde_json::Value::String(operation.status.as_str().to_owned()),
        );
    }
    let journal_id = store.journal_append(&entry).ok();

    if !cancelled && succeeded == 0 {
        return Err(first_error
            .unwrap_or_else(|| Error::Operation("no object could be deleted".to_owned())));
    }

    Ok(DeleteOutcome {
        operation_id: operation.id,
        status: operation.status,
        items,
        summary: operation.result_summary.unwrap_or_default(),
        journal_id,
        undoable: entry.undo_available,
    })
}

fn delete_one(
    store: &Store,
    quarantine: Option<&Quarantine>,
    path: &Path,
    mode: DeleteMode,
    reason: &str,
    retention_days: u32,
) -> Result<Option<Uuid>> {
    match mode {
        DeleteMode::Quarantine => {
            let quarantine = quarantine.ok_or_else(|| {
                Error::InvalidArgument("quarantine location is missing".to_owned())
            })?;
            let item = quarantine.place(store, path, reason, retention_days)?;

            Ok(Some(item.id))
        }
        DeleteMode::RecycleBin => {
            recycle::recycle(&[path.to_path_buf()])?;

            Ok(None)
        }
        DeleteMode::Permanent => {
            remove_path(path)?;

            Ok(None)
        }
    }
}

#[allow(clippy::permissions_set_readonly_false)]
fn remove_path(path: &Path) -> Result<()> {
    let verbatim = paths::to_verbatim(path)?;
    let metadata = fs::symlink_metadata(&verbatim)?;

    if metadata.file_type().is_dir() {
        return remove_directory(&verbatim);
    }

    match fs::remove_file(&verbatim) {
        Ok(()) => Ok(()),
        Err(error) if error.raw_os_error() == Some(ERROR_ACCESS_DENIED) => {
            let mut permissions = metadata.permissions();
            permissions.set_readonly(false);
            fs::set_permissions(&verbatim, permissions)?;
            fs::remove_file(&verbatim)?;

            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

#[allow(clippy::permissions_set_readonly_false)]
fn remove_directory(path: &Path) -> Result<()> {
    let mut stack = vec![path.to_path_buf()];
    let mut directories = Vec::new();

    while let Some(directory) = stack.pop() {
        directories.push(directory.clone());
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if metadata.is_dir() {
                stack.push(entry.path());
            } else {
                if metadata.permissions().readonly() {
                    let mut permissions = metadata.permissions();
                    permissions.set_readonly(false);
                    fs::set_permissions(entry.path(), permissions)?;
                }
                fs::remove_file(entry.path())?;
            }
        }
    }

    for directory in directories.into_iter().rev() {
        match fs::remove_dir(&directory) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }

    Ok(())
}

fn current_user() -> String {
    std::env::var("USERNAME").unwrap_or_else(|_| "unknown".to_owned())
}

#[cfg(test)]
mod tests {
    use freespace_core::ErrorCode;

    use super::{DeleteOptions, delete_paths};
    use crate::{Store, confirm::is_protected_path, quarantine::Quarantine};

    use crate::operation::DeleteMode;

    fn fixture() -> (tempfile::TempDir, tempfile::TempDir, Store) {
        (
            tempfile::tempdir().expect("workspace"),
            tempfile::tempdir().expect("quarantine"),
            Store::open_in_memory().expect("store"),
        )
    }

    #[test]
    fn requires_confirmation_and_keeps_the_object() {
        let (workspace, _root, store) = fixture();
        let path = workspace.path().join("keep.txt");
        std::fs::write(&path, b"payload").expect("write");

        let error = delete_paths(
            &store,
            None,
            std::slice::from_ref(&path),
            &DeleteOptions::new(DeleteMode::Permanent),
            false,
        )
        .expect_err("must refuse");

        assert_eq!(error.code(), ErrorCode::ConfirmationRequired);
        assert!(path.exists());
    }

    #[test]
    fn quarantines_objects_and_journals_undo() {
        let (workspace, root, store) = fixture();
        let first = workspace.path().join("first.txt");
        let second = workspace.path().join("second.txt");
        std::fs::write(&first, b"one").expect("first");
        std::fs::write(&second, b"two").expect("second");
        let quarantine = Quarantine::new(root.path().join("store")).expect("quarantine");

        let outcome = delete_paths(
            &store,
            Some(&quarantine),
            &[first.clone(), second.clone()],
            &DeleteOptions::new(DeleteMode::Quarantine),
            true,
        )
        .expect("delete");

        assert_eq!(outcome.summary.succeeded, 2);
        assert!(outcome.undoable);
        assert!(outcome.journal_id.is_some());
        assert!(!first.exists());
        assert!(!second.exists());
        assert_eq!(store.quarantine_list().expect("list").len(), 2);

        let entries = store.journal_list(10).expect("journal");
        assert_eq!(entries.len(), 1);
        assert!(entries[0].undo_available);
        assert_eq!(
            entries[0].details["quarantine_ids"]
                .as_array()
                .expect("ids")
                .len(),
            2
        );
    }

    #[test]
    fn permanently_removes_a_tree() {
        let (workspace, root, store) = fixture();
        let folder = workspace.path().join("folder");
        std::fs::create_dir_all(folder.join("nested")).expect("folder");
        std::fs::write(folder.join("nested").join("file.bin"), [0u8; 8]).expect("file");
        let quarantine = Quarantine::new(root.path().join("store")).expect("quarantine");

        let outcome = delete_paths(
            &store,
            Some(&quarantine),
            std::slice::from_ref(&folder),
            &DeleteOptions::new(DeleteMode::Permanent),
            true,
        )
        .expect("delete");

        assert!(outcome.first_error().is_none());
        assert!(!folder.exists());
        assert!(!outcome.undoable);
    }

    #[test]
    fn refuses_protected_paths_without_extra_confirmation() {
        let (_workspace, root, store) = fixture();
        let system_root =
            PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into()));
        let protected = system_root.join("System32").join("kernel32.dll");
        let quarantine = Quarantine::new(root.path().join("store")).expect("quarantine");
        assert!(is_protected_path(&protected));

        let error = delete_paths(
            &store,
            Some(&quarantine),
            &[protected],
            &DeleteOptions::new(DeleteMode::Quarantine),
            true,
        )
        .expect_err("must refuse");

        assert_eq!(error.code(), ErrorCode::ProtectedPath);
    }

    use std::path::PathBuf;
}
