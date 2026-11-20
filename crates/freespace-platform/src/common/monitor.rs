//! Platform-neutral per-process I/O counters.

use std::path::PathBuf;

/// Cumulative I/O counters of one process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessIo {
    /// Process identifier.
    pub pid: u32,
    /// Executable file name, or an empty string when it cannot be queried.
    pub name: String,
    /// Full image path when available.
    pub image: Option<PathBuf>,
    /// Bytes read by the process.
    pub read_bytes: u64,
    /// Bytes written by the process.
    pub write_bytes: u64,
    /// Bytes transferred in I/O operations that are neither read nor write.
    pub other_bytes: u64,
    /// Read operation count.
    pub read_operations: u64,
    /// Write operation count.
    pub write_operations: u64,
}

impl ProcessIo {
    /// Total number of bytes transferred by the process.
    #[must_use]
    pub const fn total_bytes(&self) -> u64 {
        self.read_bytes
            .saturating_add(self.write_bytes)
            .saturating_add(self.other_bytes)
    }

    /// Returns a copy with the counters cleared; useful as a baseline.
    #[must_use]
    pub fn zeroed(&self) -> Self {
        Self {
            pid: self.pid,
            name: self.name.clone(),
            image: self.image.clone(),
            read_bytes: 0,
            write_bytes: 0,
            other_bytes: 0,
            read_operations: 0,
            write_operations: 0,
        }
    }
}

/// Returns `true` when two snapshots can be compared safely.
#[must_use]
pub const fn same_process(before: &ProcessIo, after: &ProcessIo) -> bool {
    before.pid == after.pid
}
