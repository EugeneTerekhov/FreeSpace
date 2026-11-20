//! File locks and soft/force/reboot unlocking (US5, T115) — Windows only.
//!
//! This module is compiled on Windows only; `unlock_unix.rs` provides the same
//! public API and returns `E_UNSUPPORTED` because the `unlock` capability is
//! `false` elsewhere.
//!
//! Lock discovery uses the Windows Restart Manager through
//! `freespace-platform::locks`. The safety rules stay in `freespace-ops`: soft
//! release, reboot registration and journaling go through the ops layer, which
//! refuses critical processes and writes a journal entry.
//!
//! Forced termination is selective: the Restart Manager reports a set of
//! applications and `process_ids` names the ones the user confirmed. Each
//! selected process is validated against the ops plan (never critical, never the
//! current process) and then terminated through the platform layer, because
//! `freespace_ops::unlock::release` terminates every holder and cannot express a
//! selection. Force always requires elevation and an explicit confirmation.

use std::path::{Path, PathBuf};

use freespace_core::Error as CoreError;
use freespace_ops::{
    Store,
    unlock::{self as ops_unlock, UnlockMode as OpsUnlockMode, UnlockOutcome, UnlockPlan},
};
use freespace_platform::locks as platform_locks;
use uuid::Uuid;

use crate::{
    dto::{FileLock, UnlockMode, UnlockResult},
    error::{UiError, UiErrorCode},
    validation::validate_path,
};

/// Lists the applications that currently hold `path`.
///
/// # Errors
/// Returns `E_VALIDATION` for a malformed path, `E_NOT_FOUND` when the object
/// does not exist, `E_ACCESS_DENIED` when the Restart Manager cannot query it
/// and `E_IO` for other platform failures.
pub fn locks(path: &Path) -> Result<Vec<FileLock>, UiError> {
    let path = existing_path(path)?;
    let plan = ops_unlock::plan(&path).map_err(map_error)?;

    Ok(plan.locks.iter().map(file_lock).collect())
}

/// Releases the locks on `path`.
///
/// `confirmed` must be true: the interface obtains it from its confirmation
/// dialog. Force additionally requires elevation and a non-empty selection.
/// Journaling failures do not undo a release; they are reported as warnings.
///
/// # Errors
/// Returns `E_VALIDATION` without confirmation or process selection,
/// `E_ELEVATION_REQUIRED` for force without elevation, `E_ACCESS_DENIED` for a
/// critical or current process, `E_NOT_FOUND` for a missing object and `E_IO`
/// when the platform call fails.
pub fn release(
    path: &Path,
    mode: UnlockMode,
    process_ids: &[u32],
    confirmed: bool,
    elevated: bool,
    store: Option<&Store>,
) -> Result<UnlockResult, UiError> {
    let path = existing_path(path)?;
    if !confirmed {
        return Err(UiError::validation(
            "unlock.error.confirmation: releasing a lock needs confirmation",
        ));
    }

    let outcome = match mode {
        UnlockMode::Soft => {
            ops_unlock::release(&path, OpsUnlockMode::Soft, true).map_err(map_error)?
        }
        UnlockMode::Force => release_force(&path, process_ids, elevated)?,
    };

    Ok(finish(outcome, store))
}

/// Registers `path` in the reboot deletion queue.
///
/// # Errors
/// Returns `E_VALIDATION` without confirmation, `E_ELEVATION_REQUIRED` without
/// elevation, `E_ACCESS_DENIED` when the pending-rename key cannot be written
/// and `E_IO` for other failures.
pub fn schedule_reboot(
    path: &Path,
    confirmed: bool,
    elevated: bool,
    store: Option<&Store>,
) -> Result<UnlockResult, UiError> {
    let path = existing_path(path)?;
    if !confirmed {
        return Err(UiError::validation(
            "unlock.error.confirmation: scheduling a reboot needs confirmation",
        ));
    }
    if !elevated {
        return Err(UiError::new(UiErrorCode::ElevationRequired).with_details(
            "unlock.error.elevation: reboot registration needs administrator rights",
        ));
    }

    let outcome = ops_unlock::delete_on_reboot(&path, true).map_err(map_error)?;

    Ok(finish(outcome, store))
}

fn release_force(
    path: &Path,
    process_ids: &[u32],
    elevated: bool,
) -> Result<UnlockOutcome, UiError> {
    if !elevated {
        return Err(UiError::new(UiErrorCode::ElevationRequired).with_details(
            "unlock.error.elevation: terminating a process needs administrator rights",
        ));
    }
    if process_ids.is_empty() {
        return Err(UiError::validation(
            "unlock.error.selection: select at least one process",
        ));
    }

    let plan: UnlockPlan = ops_unlock::plan(path).map_err(map_error)?;
    let mut warnings = Vec::new();
    let mut terminated = Vec::new();

    for process_id in process_ids {
        let Some(lock) = plan
            .locks
            .iter()
            .find(|lock| lock.process_id == *process_id)
        else {
            return Err(UiError::validation(format!(
                "unlock.error.process_not_holding: pid {process_id} no longer holds the object"
            )));
        };
        if lock.is_critical() {
            return Err(
                UiError::new(UiErrorCode::AccessDenied).with_details(format!(
                    "unlock.error.critical: `{}` is a critical process and must not be terminated",
                    lock.process_name
                )),
            );
        }
        if *process_id == std::process::id() {
            return Err(UiError::new(UiErrorCode::AccessDenied)
                .with_details("unlock.error.self: refusing to terminate FreeSpace itself"));
        }

        match platform_locks::terminate_process(*process_id) {
            Ok(()) => terminated.push(*process_id),
            Err(error) => warnings.push(format!(
                "unlock.warning.terminate: pid {process_id}: {error}"
            )),
        }
    }

    let available = match platform_locks::is_available(path) {
        Ok(available) => available,
        Err(error) => {
            warnings.push(format!("unlock.warning.availability: {error}"));

            false
        }
    };
    if !available && terminated.is_empty() {
        warnings.push("unlock.warning.still_locked".to_owned());
    }

    Ok(UnlockOutcome {
        path: path.to_path_buf(),
        mode: OpsUnlockMode::Force,
        released: u32::try_from(terminated.len()).unwrap_or(u32::MAX),
        terminated,
        available,
        scheduled_for_reboot: false,
        warnings,
    })
}

fn finish(mut outcome: UnlockOutcome, store: Option<&Store>) -> UnlockResult {
    let journal_entry_id = store.and_then(|store| {
        let user = std::env::var("USERNAME").unwrap_or_else(|_| "unknown".to_owned());
        match ops_unlock::record(store, &outcome, &user) {
            Ok(id) => Some(id.to_string()),
            Err(error) => {
                outcome
                    .warnings
                    .push(format!("unlock.warning.journal: {error}"));

                None
            }
        }
    });
    let operation_id = journal_entry_id
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    UnlockResult {
        operation_id,
        path: outcome.path.to_string_lossy().into_owned(),
        mode: match outcome.mode {
            OpsUnlockMode::Soft => UnlockMode::Soft,
            OpsUnlockMode::Force => UnlockMode::Force,
        },
        released: outcome.released,
        terminated: outcome.terminated,
        available: outcome.available,
        scheduled_for_reboot: outcome.scheduled_for_reboot,
        warnings: outcome.warnings,
        journal_entry_id,
    }
}

fn file_lock(lock: &platform_locks::FileLock) -> FileLock {
    FileLock {
        process_id: lock.process_id,
        process_name: lock.process_name.clone(),
        service_name: lock.service_name.clone(),
        application_type: lock.application_type.as_str().to_owned(),
        access_type: "unknown".to_owned(),
        access_type_key: "unlock.access.unknown".to_owned(),
        can_soft_close: lock.can_soft_close,
        requires_reboot: lock.requires_reboot,
        critical: lock.is_critical(),
        handle_count: None,
    }
}

fn existing_path(path: &Path) -> Result<PathBuf, UiError> {
    let path = validate_path(&path.to_string_lossy())?;
    match std::fs::metadata(&path) {
        Ok(metadata) => {
            if metadata.is_file() || metadata.is_dir() {
                Ok(path)
            } else {
                Err(UiError::new(UiErrorCode::NotFound)
                    .with_details("unlock.error.not_found: unsupported object type"))
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(UiError::new(UiErrorCode::NotFound)
                .with_details(format!("unlock.error.not_found: {}", path.display())))
        }
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            Err(UiError::new(UiErrorCode::AccessDenied)
                .with_details(format!("unlock.error.access: {}", path.display())))
        }
        Err(error) => Err(UiError::new(UiErrorCode::Io).with_details(error.to_string())),
    }
}

fn map_error(error: CoreError) -> UiError {
    let code = match &error {
        CoreError::Io(io) if io.raw_os_error() == Some(5) => UiErrorCode::AccessDenied,
        CoreError::Io(io) if io.kind() == std::io::ErrorKind::NotFound => UiErrorCode::NotFound,
        _ => UiErrorCode::from_core(error.code()),
    };

    UiError::new(code).with_details(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use freespace_ops::Store;
    use freespace_platform::locks::{ApplicationType, FileLock as PlatformLock};

    use super::{file_lock, locks, release, schedule_reboot};
    use crate::dto::UnlockMode;
    use crate::error::UiErrorCode;

    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("locked.txt");
        std::fs::write(&path, b"payload").expect("write");

        (directory, path)
    }

    #[test]
    fn maps_platform_locks() {
        let lock = PlatformLock {
            process_id: 42,
            process_name: "editor.exe".to_owned(),
            service_name: String::new(),
            application_type: ApplicationType::MainWindow,
            can_soft_close: true,
            requires_reboot: false,
        };

        let view = file_lock(&lock);
        assert_eq!(view.process_id, 42);
        assert_eq!(view.application_type, "main-window");
        assert_eq!(view.access_type_key, "unlock.access.unknown");
        assert!(view.can_soft_close);
        assert!(!view.critical);
    }

    #[test]
    fn lists_no_locks_for_an_unused_file() {
        let (_directory, path) = fixture();

        let found = locks(&path).expect("locks");
        assert!(found.is_empty());
        assert_eq!(
            locks(Path::new(r"C:\definitely-missing-file.txt"))
                .expect_err("missing")
                .code,
            UiErrorCode::NotFound
        );
    }

    #[test]
    fn soft_release_requires_confirmation_and_journals() {
        let (_directory, path) = fixture();
        let store = Store::open_in_memory().expect("store");

        let error = release(&path, UnlockMode::Soft, &[], false, false, Some(&store))
            .expect_err("confirmation");
        assert_eq!(error.code, UiErrorCode::Validation);

        let result =
            release(&path, UnlockMode::Soft, &[], true, false, Some(&store)).expect("soft release");
        assert_eq!(result.mode, UnlockMode::Soft);
        assert!(result.available);
        assert!(result.journal_entry_id.is_some());
        assert_eq!(store.journal_list(5).expect("journal").len(), 1);
    }

    #[test]
    fn force_requires_elevation_and_a_selection() {
        let (_directory, path) = fixture();

        let error =
            release(&path, UnlockMode::Force, &[1234], true, false, None).expect_err("elevation");
        assert_eq!(error.code, UiErrorCode::ElevationRequired);

        let error =
            release(&path, UnlockMode::Force, &[], true, true, None).expect_err("selection");
        assert_eq!(error.code, UiErrorCode::Validation);
    }

    #[test]
    fn reboot_registration_requires_confirmation_and_elevation() {
        let (_directory, path) = fixture();

        let error = schedule_reboot(&path, false, false, None).expect_err("confirmation");
        assert_eq!(error.code, UiErrorCode::Validation);

        let error = schedule_reboot(&path, true, false, None).expect_err("elevation");
        assert_eq!(error.code, UiErrorCode::ElevationRequired);
        assert!(path.exists());
    }
}
