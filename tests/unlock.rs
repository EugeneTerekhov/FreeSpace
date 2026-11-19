//! US3 lock inspection and release tests.
//!
//! A short-lived PowerShell child process keeps a temporary file open with
//! `FileShare.None`; the test only ever touches that child and the fixture.

mod common;

use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use common::TestTree;
use freespace_ops::{
    UnlockMode,
    unlock::{self, delete_on_reboot, plan, release},
};
use freespace_platform::locks;

/// Child process that holds a fixture file open.
struct Holder {
    child: Child,
    path: PathBuf,
}

impl Holder {
    fn start(path: &Path) -> Self {
        let quoted = path.to_string_lossy().replace('\'', "''");
        let script = format!(
            "$fs = [System.IO.File]::Open('{quoted}', 'Open', 'Read', 'None'); \
             Start-Sleep -Seconds 60"
        );
        let child = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn holder process");

        Self {
            child,
            path: path.to_path_buf(),
        }
    }

    fn wait_until_locked(&self) -> Vec<locks::FileLock> {
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut locks = Vec::new();
        while Instant::now() < deadline {
            locks = locks::find_locks(&self.path).unwrap_or_default();
            if !locks.is_empty() {
                break;
            }
            thread::sleep(Duration::from_millis(200));
        }

        locks
    }
}

impl Drop for Holder {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn finds_the_process_that_holds_a_file() {
    let fixture = TestTree::new();
    let path = fixture.write("locked.txt", b"payload");
    let holder = Holder::start(&path);

    let locks = holder.wait_until_locked();

    assert!(!locks.is_empty(), "Restart Manager must see the holder");
    assert!(
        locks
            .iter()
            .any(|lock| lock.process_id == holder.child.id()),
        "holder pid {} not in {locks:?}",
        holder.child.id()
    );
    assert!(
        locks
            .iter()
            .any(|lock| lock.process_name.to_lowercase().contains("powershell")),
        "unexpected process names: {locks:?}"
    );
    assert!(!locks.iter().all(|lock| lock.is_critical()));
    assert!(!locks::is_available(&path).expect("availability"));
}

#[test]
fn force_release_terminates_the_holder() {
    let fixture = TestTree::new();
    let path = fixture.write("force.txt", b"payload");
    let holder = Holder::start(&path);
    let locks = holder.wait_until_locked();
    assert!(!locks.is_empty());

    let outcome = release(&path, UnlockMode::Force, true).expect("force release");

    assert!(outcome.available, "file must become available: {outcome:?}");
    assert!(!outcome.terminated.is_empty());
    assert!(outcome.terminated.contains(&holder.child.id()));
    assert!(locks::is_available(&path).expect("availability"));
}

#[test]
fn soft_release_requires_confirmation() {
    let fixture = TestTree::new();
    let path = fixture.write("soft.txt", b"payload");

    let error = release(&path, UnlockMode::Soft, false).expect_err("must refuse");

    assert_eq!(error.code().as_str(), "E_CONFIRMATION_REQUIRED");
    assert!(locks::is_available(&path).expect("availability"));
}

#[test]
fn plans_an_unlocked_file() {
    let fixture = TestTree::new();
    let path = fixture.write("free.txt", b"payload");

    let plan = plan(&path).expect("plan");

    assert!(plan.available);
    assert!(!plan.has_locks());
    assert!(!plan.requires_reboot);
    assert!(plan.critical.is_empty());
}

#[test]
fn refuses_reboot_deletion_without_confirmation() {
    let fixture = TestTree::new();
    let path = fixture.write("reboot.txt", b"payload");

    let error = delete_on_reboot(&path, false).expect_err("must refuse");

    assert_eq!(error.code().as_str(), "E_CONFIRMATION_REQUIRED");
    assert!(
        path.exists(),
        "nothing may be scheduled without confirmation"
    );
}

#[test]
fn records_a_soft_release_in_the_journal() {
    let fixture = TestTree::new();
    let path = fixture.write("journal.txt", b"payload");
    let outcome = release(&path, UnlockMode::Soft, true).expect("release");
    let store = freespace_ops::Store::open_in_memory().expect("store");

    let id = unlock::record(&store, &outcome, "tester").expect("journal");
    let entries = store.journal_list(5).expect("entries");

    assert!(id > 0);
    assert_eq!(entries.len(), 1);
    assert!(entries[0].summary.starts_with("unlock"));
    assert_eq!(
        entries[0].details["path"].as_str(),
        Some(path.to_string_lossy().as_ref())
    );
}
