//! Releasing file locks and scheduling deletion on reboot.
//!
//! The platform layer only reports locks and performs the two privileged
//! actions (graceful shutdown through the Restart Manager and process
//! termination). This module owns the safety rules: nothing happens without an
//! explicit confirmation, critical processes are never terminated, and every
//! attempt is journalled (FR-021…FR-024).

use std::path::{Path, PathBuf};

use freespace_core::Result;
use freespace_platform::locks::{self, FileLock};

use crate::{
    confirm::require_confirmation,
    operation::{JournalEntry, Operation, OperationKind, OperationStatus, OperationTarget},
    store::Store,
};

/// How a lock should be released.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnlockMode {
    /// Ask applications to close the object gracefully.
    Soft,
    /// Terminate the holding processes.
    Force,
}

impl UnlockMode {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Soft => "soft",
            Self::Force => "force",
        }
    }

    /// Parses a command-line value.
    #[must_use]
    pub fn parse_value(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "soft" => Some(Self::Soft),
            "force" => Some(Self::Force),
            _ => None,
        }
    }
}

/// What the user should see before confirming an unlock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockPlan {
    /// Inspected object.
    pub path: PathBuf,
    /// Processes that hold the object.
    pub locks: Vec<FileLock>,
    /// Whether the object is currently accessible.
    pub available: bool,
    /// Whether a reboot is required to release the object.
    pub requires_reboot: bool,
    /// Whether at least one holder can be closed gracefully.
    pub can_soft_close: bool,
    /// Whether at least one holder can be terminated.
    pub can_force_close: bool,
    /// Critical holders that must not be touched.
    pub critical: Vec<FileLock>,
}

impl UnlockPlan {
    /// Returns `true` when at least one lock was found.
    #[must_use]
    pub fn has_locks(&self) -> bool {
        !self.locks.is_empty()
    }
}

/// Result of an unlock attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockOutcome {
    /// Inspected object.
    pub path: PathBuf,
    /// Applied mode.
    pub mode: UnlockMode,
    /// Processes that were asked to close the object.
    pub released: u32,
    /// Processes that were terminated.
    pub terminated: Vec<u32>,
    /// Whether the object is accessible afterwards.
    pub available: bool,
    /// Whether deletion was put into the reboot queue.
    pub scheduled_for_reboot: bool,
    /// Non-fatal notes.
    pub warnings: Vec<String>,
}

/// Inspects the locks held on `path`.
///
/// # Errors
/// Returns an error when the Restart Manager cannot be queried.
pub fn plan(path: &Path) -> Result<UnlockPlan> {
    let locks = locks::find_locks(path)?;
    let available = locks::is_available(path)?;
    let requires_reboot = locks.iter().any(|lock| lock.requires_reboot);
    let critical: Vec<FileLock> = locks
        .iter()
        .filter(|lock| lock.is_critical())
        .cloned()
        .collect();
    let can_force_close = locks
        .iter()
        .any(|lock| lock.application_type.is_terminable());
    let can_soft_close = locks.iter().any(|lock| lock.can_soft_close);

    Ok(UnlockPlan {
        path: path.to_path_buf(),
        locks,
        available,
        requires_reboot,
        can_soft_close,
        can_force_close,
        critical,
    })
}

/// Releases the locks on `path`.
///
/// # Errors
/// Returns [`Error::ConfirmationRequired`] without `confirmed`, and
/// [`Error::Locked`] when the object stays locked after the operation.
pub fn release(path: &Path, mode: UnlockMode, confirmed: bool) -> Result<UnlockOutcome> {
    require_confirmation(confirmed)?;

    let plan = plan(path)?;
    let mut warnings = Vec::new();
    let mut terminated = Vec::new();
    let mut released = 0u32;

    match mode {
        UnlockMode::Soft => {
            if plan.can_soft_close {
                released = locks::release_soft(path)?;
            } else if !plan.locks.is_empty() {
                warnings.push(
                    "the Restart Manager cannot close these applications gracefully".to_owned(),
                );
            }
        }
        UnlockMode::Force => {
            for lock in &plan.locks {
                if !lock.application_type.is_terminable() {
                    warnings.push(format!(
                        "refused to terminate `{}` ({}, pid {})",
                        lock.process_name,
                        lock.application_type.as_str(),
                        lock.process_id
                    ));
                    continue;
                }
                match locks::terminate_process(lock.process_id) {
                    Ok(()) => terminated.push(lock.process_id),
                    Err(error) => {
                        warnings.push(format!("cannot terminate pid {}: {error}", lock.process_id))
                    }
                }
            }
            released = u32::try_from(terminated.len()).unwrap_or(0);
        }
    }

    let available = wait_available(path, std::time::Duration::from_secs(3));
    if !available && plan.locks.is_empty() {
        warnings.push("the object stays unavailable although no process is registered".to_owned());
    }

    Ok(UnlockOutcome {
        path: path.to_path_buf(),
        mode,
        released,
        terminated,
        available,
        scheduled_for_reboot: false,
        warnings,
    })
}

/// Registers `path` in the Windows reboot deletion queue.
///
/// # Errors
/// Returns [`Error::ConfirmationRequired`] without `confirmed` and an I/O error
/// when the pending-rename key cannot be written.
pub fn delete_on_reboot(path: &Path, confirmed: bool) -> Result<UnlockOutcome> {
    require_confirmation(confirmed)?;
    locks::schedule_delete_on_reboot(path)?;

    Ok(UnlockOutcome {
        path: path.to_path_buf(),
        mode: UnlockMode::Force,
        released: 0,
        terminated: Vec::new(),
        available: false,
        scheduled_for_reboot: true,
        warnings: vec!["the object will be removed before the next graphical logon".to_owned()],
    })
}

fn wait_available(path: &Path, timeout: std::time::Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if locks::is_available(path).unwrap_or(false) {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

/// Appends an unlock attempt to the operation journal.
///
/// # Errors
/// Returns an error when the journal cannot be written.
pub fn record(store: &Store, outcome: &UnlockOutcome, user: &str) -> Result<u64> {
    let target = OperationTarget::new(outcome.path.clone(), 0);
    let kind = if outcome.scheduled_for_reboot {
        OperationKind::DeleteOnReboot
    } else {
        OperationKind::Unlock
    };
    let mut operation = Operation::new(kind, vec![target]);
    let mut status = OperationStatus::Completed;
    if !outcome.available && !outcome.scheduled_for_reboot {
        status = OperationStatus::Failed;
    }
    operation.confirm()?;
    operation.begin()?;
    operation.targets[0].status = if status == OperationStatus::Completed {
        crate::operation::TargetStatus::Succeeded
    } else {
        crate::operation::TargetStatus::Failed
    };
    operation.finish(status)?;

    let mut entry = JournalEntry::from_operation(&operation, user);
    entry.summary = format!(
        "unlock ({}): {} released, {} terminated{}",
        outcome.mode.as_str(),
        outcome.released,
        outcome.terminated.len(),
        if outcome.scheduled_for_reboot {
            ", scheduled for reboot"
        } else {
            ""
        }
    );
    entry.details = serde_json::json!({
        "path": outcome.path.to_string_lossy(),
        "mode": outcome.mode.as_str(),
        "released": outcome.released,
        "terminated": outcome.terminated,
        "available": outcome.available,
        "scheduled_for_reboot": outcome.scheduled_for_reboot,
        "warnings": outcome.warnings,
    });

    store.journal_append(&entry)
}

#[cfg(test)]
mod tests {
    use super::{UnlockMode, delete_on_reboot, plan, record, release};
    use crate::store::Store;

    fn unlock_path() -> (tempfile::TempDir, std::path::PathBuf) {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("locked.txt");
        std::fs::write(&path, b"payload").expect("write file");

        (directory, path)
    }

    #[test]
    fn reports_an_unlocked_file() {
        let (_directory, path) = unlock_path();

        let plan = plan(&path).expect("plan");

        assert!(plan.available);
        assert!(!plan.has_locks());
        assert!(!plan.requires_reboot);
        assert!(plan.critical.is_empty());
        assert_eq!(UnlockMode::parse_value("soft"), Some(UnlockMode::Soft));
        assert_eq!(UnlockMode::parse_value("force"), Some(UnlockMode::Force));
        assert_eq!(UnlockMode::parse_value("magic"), None);
        assert_eq!(UnlockMode::Soft.as_str(), "soft");
    }

    #[test]
    fn requires_confirmation_for_release_and_reboot() {
        let (_directory, path) = unlock_path();

        let error = release(&path, UnlockMode::Soft, false).expect_err("must refuse");
        assert_eq!(error.code().as_str(), "E_CONFIRMATION_REQUIRED");

        let error = delete_on_reboot(&path, false).expect_err("must refuse");
        assert_eq!(error.code().as_str(), "E_CONFIRMATION_REQUIRED");
    }

    #[test]
    fn soft_release_of_an_unlocked_file_succeeds() {
        let (_directory, path) = unlock_path();

        let outcome = release(&path, UnlockMode::Soft, true).expect("release");

        assert_eq!(outcome.released, 0);
        assert!(outcome.available);
        assert!(outcome.terminated.is_empty());
        assert!(!outcome.scheduled_for_reboot);
        assert!(!outcome.path.as_os_str().is_empty());
    }

    #[test]
    fn journals_unlock_attempts() {
        let (_directory, path) = unlock_path();
        let outcome = release(&path, UnlockMode::Soft, true).expect("release");
        let store = Store::open_in_memory().expect("store");

        let id = record(&store, &outcome, "tester").expect("record");
        let entries = store.journal_list(10).expect("list");

        assert!(id > 0);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].summary.starts_with("unlock"));
        assert_eq!(entries[0].details["mode"], "soft");
        assert_eq!(entries[0].details["available"], true);
    }
}
