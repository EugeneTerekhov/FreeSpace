//! `freespace unlock` — inspect and release file locks.

use std::path::Path;

use freespace_core::Result;
use freespace_ops::{
    Store,
    unlock::{self, UnlockMode, UnlockOutcome},
};
use freespace_platform::paths;
use serde_json::{Value, json};

use super::Outcome;

/// Runs `unlock` in list, soft or force mode.
///
/// `store` receives a journal entry when the caller has a database open; tests
/// pass `None` to avoid touching the user profile.
///
/// # Errors
/// Returns an error when the path is locked beyond recovery or confirmation is
/// missing.
pub fn run(
    path: &Path,
    mode: Option<UnlockMode>,
    confirmed: bool,
    store: Option<&Store>,
) -> Result<Outcome> {
    match mode {
        None => list(path),
        Some(mode) => unlock::release(path, mode, confirmed).map(|outcome| report(outcome, store)),
    }
}

/// Runs `unlock --reboot` (deletion before the next graphical logon).
///
/// # Errors
/// Returns an error without confirmation or when the queue cannot be updated.
pub fn run_delete_on_reboot(
    path: &Path,
    confirmed: bool,
    store: Option<&Store>,
) -> Result<Outcome> {
    unlock::delete_on_reboot(path, confirmed).map(|outcome| report(outcome, store))
}

fn list(path: &Path) -> Result<Outcome> {
    let plan = unlock::plan(path)?;
    let locks: Vec<Value> = plan
        .locks
        .iter()
        .map(|lock| {
            json!({
                "process_id": lock.process_id,
                "process_name": lock.process_name,
                "service_name": lock.service_name,
                "application_type": lock.application_type.as_str(),
                "can_soft_close": lock.can_soft_close,
                "requires_reboot": lock.requires_reboot,
                "critical": lock.is_critical(),
            })
        })
        .collect();

    let mut human = format!(
        "{}: {}\n",
        paths::display(&plan.path),
        if plan.available {
            "available"
        } else {
            "locked"
        }
    );
    if plan.locks.is_empty() {
        human.push_str("no locking process found\n");
    }
    for lock in &plan.locks {
        human.push_str(&format!(
            "  pid {:<6} {:<20} {}{}\n",
            lock.process_id,
            lock.process_name,
            lock.application_type.as_str(),
            if lock.is_critical() {
                "  [critical]"
            } else {
                ""
            }
        ));
    }
    if plan.requires_reboot {
        human.push_str("a reboot is required to release this object\n");
    }

    let mut warnings = Vec::new();
    if !plan.available && plan.locks.is_empty() {
        warnings.push("the object is not accessible although no process is registered".to_owned());
    }

    Ok(Outcome::new(
        json!({
            "path": paths::display(&plan.path),
            "available": plan.available,
            "requires_reboot": plan.requires_reboot,
            "can_soft_close": plan.can_soft_close,
            "can_force_close": plan.can_force_close,
            "locks": locks,
        }),
        human,
    )
    .with_warnings(warnings))
}

fn report(outcome: UnlockOutcome, store: Option<&Store>) -> Outcome {
    if let Some(store) = store {
        let user = std::env::var("USERNAME").unwrap_or_else(|_| "unknown".to_owned());
        let _ = unlock::record(store, &outcome, &user);
    }

    let human = if outcome.scheduled_for_reboot {
        format!(
            "{}: scheduled for deletion before the next logon\n",
            paths::display(&outcome.path)
        )
    } else {
        format!(
            "{}: {} released, {} terminated, {}\n",
            paths::display(&outcome.path),
            outcome.released,
            outcome.terminated.len(),
            if outcome.available {
                "available"
            } else {
                "still locked"
            }
        )
    };

    Outcome::new(
        json!({
            "path": paths::display(&outcome.path),
            "mode": outcome.mode.as_str(),
            "released": outcome.released,
            "terminated": outcome.terminated,
            "available": outcome.available,
            "scheduled_for_reboot": outcome.scheduled_for_reboot,
        }),
        human,
    )
    .with_warnings(outcome.warnings)
}

#[cfg(test)]
mod tests {
    use super::run;
    use freespace_ops::UnlockMode;

    #[test]
    fn lists_locks_as_json() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("file.txt");
        std::fs::write(&path, b"payload").expect("file");

        let outcome = run(&path, None, false, None).expect("list");

        assert_eq!(outcome.data["available"].as_bool(), Some(true));
        assert!(outcome.data["locks"].as_array().expect("locks").is_empty());
        assert!(outcome.human.contains("available"));
    }

    #[test]
    fn requires_confirmation_for_soft_release() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("file.txt");
        std::fs::write(&path, b"payload").expect("file");

        let error = run(&path, Some(UnlockMode::Soft), false, None).expect_err("must refuse");

        assert_eq!(error.exit_code(), 5);
    }

    #[test]
    fn soft_release_reports_availability() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("file.txt");
        std::fs::write(&path, b"payload").expect("file");

        let outcome = run(&path, Some(UnlockMode::Soft), true, None).expect("release");

        assert_eq!(outcome.data["available"].as_bool(), Some(true));
        assert_eq!(outcome.data["mode"].as_str(), Some("soft"));
    }
}
