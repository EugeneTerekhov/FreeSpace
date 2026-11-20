//! Cross-process instance lock backed by `flock`.
//!
//! One lock file in the portable `data` directory keeps two FreeSpace instances
//! from writing the same journal and settings. `flock` is advisory but attaches
//! to the open file description, so it works without a daemon and is released
//! automatically when the guard, and with it the file, is dropped.

use std::{
    fs::{File, OpenOptions},
    io::{Seek as _, SeekFrom, Write as _},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use freespace_core::{Error, Result};
use rustix::{
    fs::{FlockOperation, flock},
    io::Errno,
};

use crate::InstanceLockGuard;

/// Polling interval while another process holds the lock.
const RETRY_INTERVAL: Duration = Duration::from_millis(50);

/// Acquires the lock file, waiting up to `timeout`.
///
/// # Errors
/// Returns [`Error::Locked`] when the timeout expires, [`Error::Io`] when the
/// file cannot be opened or locked for another reason.
pub fn acquire(path: &Path, timeout: Duration) -> Result<Box<dyn InstanceLockGuard>> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(Error::Io)?;
    let deadline = Instant::now() + timeout;

    loop {
        match flock(&file, FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => {
                return Ok(Box::new(Guard {
                    path: path.to_path_buf(),
                    file,
                }));
            }
            Err(error) if is_contended(error) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(Error::Locked(format!(
                        "another FreeSpace instance holds `{}`",
                        path.display()
                    )));
                }
                thread::sleep(remaining.min(RETRY_INTERVAL));
            }
            Err(error) => return Err(Error::Io(error.into())),
        }
    }
}

fn is_contended(error: Errno) -> bool {
    error == Errno::WOULDBLOCK || error == Errno::AGAIN
}

struct Guard {
    path: PathBuf,
    file: File,
}

impl InstanceLockGuard for Guard {
    fn path(&self) -> &Path {
        &self.path
    }

    fn write_metadata(&mut self, contents: &str) -> Result<()> {
        self.file.seek(SeekFrom::Start(0)).map_err(Error::Io)?;
        self.file
            .write_all(contents.as_bytes())
            .map_err(Error::Io)?;
        self.file
            .set_len(u64::try_from(contents.len()).unwrap_or(u64::MAX))
            .map_err(Error::Io)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::acquire;

    #[test]
    fn holds_the_lock_and_writes_metadata() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("freespace.lock");

        let mut guard = acquire(&path, Duration::from_millis(10)).expect("lock");

        assert_eq!(guard.path(), path);
        guard
            .write_metadata("pid=42\nversion=2.0.0\n")
            .expect("metadata");
        drop(guard);

        assert_eq!(
            std::fs::read_to_string(&path).expect("read lock file"),
            "pid=42\nversion=2.0.0\n"
        );
    }

    #[test]
    fn times_out_when_another_guard_holds_the_lock() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("freespace.lock");
        let _guard = acquire(&path, Duration::from_millis(10)).expect("first lock");

        let error = match acquire(&path, Duration::from_millis(20)) {
            Ok(_) => panic!("the second call must time out"),
            Err(error) => error,
        };

        assert_eq!(error.code(), freespace_core::ErrorCode::Locked);
    }
}
