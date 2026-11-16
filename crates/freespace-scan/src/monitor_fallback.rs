//! Write-monitor fallback used when the ETW kernel session is unavailable.
//!
//! The fallback combines two cheaper sources:
//!
//! * `GetProcessIoCounters` gives exact byte counts per process, but no paths;
//! * the USN journal tail gives paths of changed files, but no process ids.
//!
//! [`FallbackMonitor`] computes rates from two counter snapshots, while
//! [`tail_paths`] turns new journal records into path hints. Both pieces are
//! pure enough to test without an elevated session.

use std::collections::HashMap;
use std::path::Path;

use freespace_core::{Result, Timestamp, model::JournalState};
use freespace_platform::{
    mft::{
        self, USN_REASON_DATA_EXTEND, USN_REASON_DATA_OVERWRITE, USN_REASON_DATA_TRUNCATION,
        UsnRecord,
    },
    process_io::{self, ProcessIo},
};

/// Maximum number of USN paths returned by one poll.
pub const MAX_TAIL_PATHS: usize = 1_000;

/// Write rate of one process between two counter snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessWrite {
    /// Process identifier.
    pub pid: u32,
    /// Executable name, or an empty string when it is unknown.
    pub process: String,
    /// Bytes written since the previous snapshot.
    pub bytes: u64,
    /// Write operations since the previous snapshot.
    pub operations: u64,
}

/// Path hint extracted from the USN journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathWrite {
    /// File reference number.
    pub file_ref: u64,
    /// Parent directory reference number.
    pub parent_ref: u64,
    /// File name stored in the record.
    pub name: String,
    /// When the change was recorded.
    pub timestamp: Timestamp,
    /// Bytes appended, when the journal record reports it.
    pub bytes: u64,
}

/// Polling monitor backed by process I/O counters.
#[derive(Debug, Default)]
pub struct FallbackMonitor {
    previous: HashMap<u32, ProcessIo>,
}

impl FallbackMonitor {
    /// Creates a monitor without a baseline; the first [`Self::poll`] returns no
    /// rates and only records the baseline.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Discards the baseline, for example after a pause.
    pub fn reset(&mut self) {
        self.previous.clear();
    }

    /// Polls the counters and returns the writes since the previous poll.
    ///
    /// # Errors
    /// Returns [`freespace_core::Error`] when the process list cannot be read.
    pub fn poll(&mut self) -> Result<Vec<ProcessWrite>> {
        let snapshot = process_io::snapshot()?;
        let writes = if self.previous.is_empty() {
            Vec::new()
        } else {
            let mut writes = write_deltas(self.previous.values(), &snapshot);
            writes.retain(|write| write.bytes > 0);
            writes.sort_by(|left, right| right.bytes.cmp(&left.bytes));
            writes
        };
        self.previous = snapshot
            .into_iter()
            .map(|process| (process.pid, process))
            .collect();

        Ok(writes)
    }

    /// Number of processes in the baseline.
    #[must_use]
    pub fn tracked(&self) -> usize {
        self.previous.len()
    }
}

/// Computes write deltas between two counter snapshots.
///
/// Processes that exited, appeared or restarted are ignored because their
/// counters cannot be compared: a lower counter than before is treated as a
/// restart instead of an unsigned underflow.
#[must_use]
pub fn write_deltas<'a, I>(before: I, after: &[ProcessIo]) -> Vec<ProcessWrite>
where
    I: IntoIterator<Item = &'a ProcessIo>,
{
    let before: HashMap<u32, &ProcessIo> = before
        .into_iter()
        .map(|process| (process.pid, process))
        .collect();

    after
        .iter()
        .filter_map(|current| {
            let previous = before.get(&current.pid)?;
            let bytes = current.write_bytes.checked_sub(previous.write_bytes)?;
            let operations = current
                .write_operations
                .checked_sub(previous.write_operations)
                .unwrap_or_default();
            if bytes == 0 && operations == 0 {
                return None;
            }

            Some(ProcessWrite {
                pid: current.pid,
                process: current.name.clone(),
                bytes,
                operations,
            })
        })
        .collect()
}

/// Extracts a write hint from a USN record.
#[must_use]
pub fn usn_write(record: &UsnRecord) -> Option<PathWrite> {
    let reason = record.reason
        & (USN_REASON_DATA_EXTEND | USN_REASON_DATA_OVERWRITE | USN_REASON_DATA_TRUNCATION);
    if reason == 0 || record.name.is_empty() {
        return None;
    }

    Some(PathWrite {
        file_ref: record.file_ref,
        parent_ref: record.parent_ref,
        name: record.name.clone(),
        timestamp: record.timestamp,
        bytes: 0,
    })
}

/// Reads the journal tail and returns paths of files whose data changed.
///
/// The journal state is advanced regardless of how many records are returned, so
/// the next call only reports new writes.
///
/// # Errors
/// Returns [`freespace_core::Error`] when the volume or journal cannot be read.
pub fn tail_paths(root: &Path, state: &mut JournalState) -> Result<Vec<PathWrite>> {
    let journal = mft::read_journal(root, *state)?;
    *state = journal.state;

    Ok(journal
        .records
        .iter()
        .filter_map(usn_write)
        .take(MAX_TAIL_PATHS)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::{FallbackMonitor, ProcessWrite, usn_write, write_deltas};
    use freespace_core::Timestamp;
    use freespace_platform::{
        mft::{USN_REASON_DATA_EXTEND, USN_REASON_FILE_CREATE, UsnRecord},
        process_io::ProcessIo,
    };

    fn process(pid: u32, written: u64, operations: u64) -> ProcessIo {
        ProcessIo {
            pid,
            name: format!("proc-{pid}"),
            image: None,
            read_bytes: 0,
            write_bytes: written,
            other_bytes: 0,
            read_operations: 0,
            write_operations: operations,
        }
    }

    #[test]
    fn computes_deltas_and_sorts_nothing() {
        let before = [process(1, 100, 1), process(2, 0, 0)];
        let after = [process(1, 400, 3), process(2, 0, 0), process(3, 999, 9)];

        let deltas = write_deltas(&before, &after);

        assert_eq!(
            deltas,
            vec![ProcessWrite {
                pid: 1,
                process: "proc-1".to_owned(),
                bytes: 300,
                operations: 2,
            }]
        );
    }

    #[test]
    fn ignores_restarts_and_underflows() {
        let before = [process(1, 100, 2)];
        let after = [process(1, 10, 1)];

        assert!(write_deltas(&before, &after).is_empty());
    }

    #[test]
    fn first_poll_only_records_the_baseline() {
        let mut monitor = FallbackMonitor::new();

        let first = monitor.poll().expect("first poll");
        let _second = monitor.poll().expect("second poll");

        assert!(first.is_empty(), "the first poll has no baseline");
        assert!(monitor.tracked() > 0);
        monitor.reset();
        assert_eq!(monitor.tracked(), 0);
    }

    fn record(reason: u32, name: &str) -> UsnRecord {
        UsnRecord {
            file_ref: 42,
            parent_ref: 5,
            usn: 1,
            timestamp: Timestamp::from_millis(1_700_000_000_000),
            reason,
            attributes: freespace_core::tree::FileAttributes::empty(),
            name: name.to_owned(),
        }
    }

    #[test]
    fn keeps_only_data_changes() {
        assert!(usn_write(&record(USN_REASON_FILE_CREATE, "a.txt")).is_none());
        assert!(usn_write(&record(USN_REASON_DATA_EXTEND, "")).is_none());

        let write = usn_write(&record(USN_REASON_DATA_EXTEND, "a.txt")).expect("write");

        assert_eq!(write.name, "a.txt");
        assert_eq!(write.file_ref, 42);
        assert_eq!(write.bytes, 0);
    }
}
