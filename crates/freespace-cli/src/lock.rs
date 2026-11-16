//! Cross-process run lock and queue for the CLI (FR-078).
//!
//! FreeSpace serialises command runs through an advisory lock next to its
//! SQLite store. A second process waits up to `--lock-timeout` seconds and then
//! fails with the documented `E_LOCKED` code instead of corrupting shared state
//! or competing for disks.

use std::{path::PathBuf, time::Duration};

use freespace_core::{Result, Timestamp};
use freespace_platform::file_lock::FileLock;
use serde_json::json;

/// Default wait time before a queued run fails.
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Resolves the lock file path.
///
/// The path lives beside `freespace.db`; temporary directories are the fallback
/// when the profile directory is unavailable.
#[must_use]
pub fn default_lock_path() -> PathBuf {
    freespace_ops::store::default_path()
        .map(|path| path.with_file_name("cli.lock"))
        .unwrap_or_else(|_| std::env::temp_dir().join("FreeSpace").join("cli.lock"))
}

/// Acquires the run lock, waiting up to `timeout_secs`.
///
/// # Errors
/// Returns [`freespace_core::Error::Locked`] when another process keeps the lock
/// for longer than the timeout.
pub fn acquire(timeout_secs: u64) -> Result<FileLock> {
    acquire_at(&default_lock_path(), timeout_secs)
}

/// Acquires a lock at an explicit path; used by tests and embedding tools.
///
/// # Errors
/// Returns [`freespace_core::Error::Locked`] when the timeout expires.
pub fn acquire_at(path: &std::path::Path, timeout_secs: u64) -> Result<FileLock> {
    FileLock::acquire(path, Duration::from_secs(timeout_secs))
}

/// Writes diagnostic metadata into the held lock file.
///
/// # Errors
/// Returns an error when the lock file cannot be rewritten.
pub fn describe(lock: &mut FileLock, command: &str) -> Result<()> {
    let metadata = json!({
        "pid": std::process::id(),
        "command": command,
        "started_at": Timestamp::now().as_millis(),
    });
    lock.write_metadata(&metadata.to_string())
}

#[cfg(test)]
mod tests {
    use super::{acquire_at, default_lock_path};

    #[test]
    fn lock_lives_beside_the_store_or_in_temp() {
        let path = default_lock_path();

        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("cli.lock")
        );
    }

    #[test]
    fn queues_on_an_explicit_path() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("cli.lock");
        let lock = acquire_at(&path, 1).expect("lock");

        assert!(lock.path().exists());
        drop(lock);
        acquire_at(&path, 1).expect("released");
    }
}
