//! Cross-process advisory file lock used to serialise FreeSpace CLI runs
//! (FR-078).
//!
//! The lock is a real `LockFileEx` byte-range lock on a local file, so it is
//! released automatically when the process exits, including after a crash.
//! Waiting runs queue behind the holder instead of failing immediately.

use std::{
    fs::{File, OpenOptions},
    io::Write as _,
    os::windows::io::AsRawHandle,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use freespace_core::{Error, Result};
use windows::{
    Win32::{
        Foundation::{ERROR_LOCK_VIOLATION, HANDLE},
        Storage::FileSystem::{
            LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY, LockFileEx, UnlockFileEx,
        },
        System::IO::OVERLAPPED,
    },
    core::Error as WindowsError,
};

use super::win;

/// Interval between lock attempts while queueing.
const RETRY_INTERVAL: Duration = Duration::from_millis(100);

/// Held exclusive lock. Releasing it happens in [`Drop`].
#[derive(Debug)]
pub struct FileLock {
    file: File,
    path: PathBuf,
}

impl FileLock {
    /// Tries to take the lock without waiting.
    ///
    /// Returns `Ok(None)` when another process holds the lock.
    ///
    /// # Errors
    /// Returns an error when the lock file cannot be created or locked for a
    /// reason other than contention.
    pub fn try_acquire(path: &Path) -> Result<Option<Self>> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;

        if lock_handle(&file)? {
            Ok(Some(Self {
                file,
                path: path.to_path_buf(),
            }))
        } else {
            Ok(None)
        }
    }

    /// Waits up to `timeout` for the lock.
    ///
    /// A zero timeout is equivalent to [`Self::try_acquire`].
    ///
    /// # Errors
    /// Returns [`Error::Locked`] when the timeout expires and an error when the
    /// lock file itself cannot be used.
    pub fn acquire(path: &Path, timeout: Duration) -> Result<Self> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(lock) = Self::try_acquire(path)? {
                return Ok(lock);
            }
            if Instant::now() >= deadline {
                return Err(Error::Locked(format!(
                    "another FreeSpace instance holds `{}`",
                    path.display()
                )));
            }
            thread::sleep(RETRY_INTERVAL);
        }
    }

    /// Path of the lock file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Writes a small diagnostic record into the lock file.
    ///
    /// The record is informational only: ownership is enforced by the byte-range
    /// lock, never by the file contents.
    ///
    /// # Errors
    /// Returns an error when the file cannot be rewritten.
    pub fn write_metadata(&mut self, contents: &str) -> Result<()> {
        self.file.set_len(0)?;
        self.file.write_all(contents.as_bytes())?;
        self.file.flush()?;
        // Rewriting moved the file pointer; the byte-range lock is absolute and
        // remains valid.

        Ok(())
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let mut overlapped = OVERLAPPED::default();
        // SAFETY: the handle is valid for the lifetime of `self.file`, the
        // range matches the one acquired in `lock_handle`, and `overlapped` is
        // valid for the duration of the call.
        let _ = unsafe {
            UnlockFileEx(
                HANDLE(self.file.as_raw_handle()),
                None,
                1,
                0,
                &mut overlapped,
            )
        };
    }
}

fn lock_handle(file: &File) -> Result<bool> {
    let mut overlapped = OVERLAPPED::default();
    // SAFETY: the handle is valid, the range is one byte and `overlapped` is
    // valid for the duration of the call.
    let result = unsafe {
        LockFileEx(
            HANDLE(file.as_raw_handle()),
            LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
            None,
            1,
            0,
            &mut overlapped,
        )
    };

    match result {
        Ok(()) => Ok(true),
        Err(error) => {
            let io_error = win::win_error(&error);
            if io_error.raw_os_error() == Some(ERROR_LOCK_VIOLATION.0 as i32) {
                Ok(false)
            } else {
                Err(Error::Io(io_error))
            }
        }
    }
}

/// Returns `true` when the error is a lock-contention error.
#[must_use]
pub fn is_contention(error: &WindowsError) -> bool {
    win::win_error(error).raw_os_error() == Some(ERROR_LOCK_VIOLATION.0 as i32)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::FileLock;

    #[test]
    fn excludes_a_second_holder_and_releases_on_drop() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("cli.lock");

        let lock = FileLock::try_acquire(&path)
            .expect("lock")
            .expect("first holder");
        assert!(
            FileLock::try_acquire(&path)
                .expect("second attempt")
                .is_none(),
            "a second handle must not acquire the lock"
        );
        assert!(
            FileLock::acquire(&path, Duration::ZERO).is_err(),
            "queueing with a zero timeout must fail"
        );

        drop(lock);
        let again = FileLock::try_acquire(&path)
            .expect("lock after drop")
            .expect("released");
        assert_eq!(again.path(), path);
    }

    #[test]
    fn writes_diagnostic_metadata() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("cli.lock");
        let mut lock = FileLock::try_acquire(&path).expect("lock").expect("holder");

        lock.write_metadata("{\"pid\":1}").expect("metadata");
        drop(lock);
        let contents = std::fs::read_to_string(&path).expect("read");

        assert_eq!(contents, "{\"pid\":1}");
    }
}
