//! Per-process I/O counters used by the write monitor fallback (US20).
//!
//! `GetProcessIoCounters` reports cumulative byte counts per process, so the
//! caller derives the current write rate by comparing two snapshots. It needs no
//! elevation for processes owned by the current user and therefore keeps the
//! monitor useful when the ETW kernel session is unavailable.

use std::{ffi::OsString, os::windows::ffi::OsStringExt as _, path::PathBuf};

use freespace_core::{Error, Result};
use windows::{
    Win32::System::{
        ProcessStatus::EnumProcesses,
        Threading::{
            GetProcessIoCounters, IO_COUNTERS, OpenProcess, PROCESS_NAME_FORMAT,
            PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
        },
    },
    core::PWSTR,
};

use super::win::{self, Handle};

pub use crate::common::monitor::{ProcessIo, same_process};

/// Takes a snapshot of the I/O counters of every process the caller can open.
///
/// Processes that cannot be opened (services owned by other users, protected
/// processes) are skipped instead of failing the whole snapshot.
///
/// # Errors
/// Returns [`Error::Io`] when the process list itself cannot be enumerated.
pub fn snapshot() -> Result<Vec<ProcessIo>> {
    let mut capacity = 1024usize;

    loop {
        let mut pids = vec![0u32; capacity];
        let mut needed = 0u32;
        // SAFETY: the buffer is writable for `capacity * size_of::<u32>()` bytes.
        let result = unsafe {
            EnumProcesses(
                pids.as_mut_ptr(),
                u32::try_from(pids.len() * size_of::<u32>()).unwrap_or(u32::MAX),
                &mut needed,
            )
        };
        result.map_err(|error| Error::Io(win::win_error(&error)))?;

        let used = usize::try_from(needed / size_of::<u32>() as u32).unwrap_or(capacity);
        if used < capacity {
            pids.truncate(used.min(capacity));

            return Ok(pids
                .into_iter()
                .filter(|pid| *pid != 0)
                .filter_map(|pid| counters_for(pid).ok())
                .collect());
        }

        capacity = capacity.saturating_mul(2);
        if capacity > 1_048_576 {
            return Err(Error::Operation(
                "the process list is unreasonably large".to_owned(),
            ));
        }
    }
}

/// Reads the counters of a single process.
///
/// # Errors
/// Returns [`Error::Io`] when the process cannot be opened or queried.
pub fn counters_for(pid: u32) -> Result<ProcessIo> {
    // SAFETY: OpenProcess has no pointer preconditions; the handle is owned.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }
        .map_err(|error| Error::Io(win::win_error(&error)))?;
    let handle = Handle::from_raw(handle)?;
    let mut counters = IO_COUNTERS::default();
    // SAFETY: `counters` is writable for the duration of the call.
    unsafe { GetProcessIoCounters(handle.raw(), &raw mut counters) }
        .map_err(|error| Error::Io(win::win_error(&error)))?;

    let (name, image) = process_image(handle.raw());

    Ok(ProcessIo {
        pid,
        name,
        image,
        read_bytes: counters.ReadTransferCount,
        write_bytes: counters.WriteTransferCount,
        other_bytes: counters.OtherTransferCount,
        read_operations: counters.ReadOperationCount,
        write_operations: counters.WriteOperationCount,
    })
}

fn process_image(handle: windows::Win32::Foundation::HANDLE) -> (String, Option<PathBuf>) {
    let mut buffer = vec![0u16; 32_768];
    let mut length = u32::try_from(buffer.len()).unwrap_or(0);
    // SAFETY: the buffer is writable for `length` UTF-16 code units.
    let result = unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
    };

    if result.is_err() {
        return (String::new(), None);
    }

    buffer.truncate(usize::try_from(length).unwrap_or(0));
    let image = OsString::from_wide(&buffer);
    let path = PathBuf::from(&image);
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();

    (name, Some(path))
}

#[cfg(test)]
mod tests {
    use super::{ProcessIo, counters_for, snapshot};

    #[test]
    fn snapshots_the_current_processes() {
        let processes = snapshot().expect("snapshot");

        assert!(
            processes
                .iter()
                .any(|process| process.pid == std::process::id()),
            "the current process must be visible"
        );
        for process in processes {
            assert_ne!(process.pid, 0);
            assert!(process.write_bytes <= process.total_bytes());
        }
    }

    #[test]
    fn reads_the_counters_of_the_current_process() {
        let counters = counters_for(std::process::id()).expect("own counters");

        assert_eq!(counters.pid, std::process::id());
        assert!(!counters.name.is_empty());
        assert!(counters.image.is_some());
        assert_eq!(counters.zeroed().write_bytes, 0);
        assert!(super::same_process(&counters, &counters.zeroed()));
    }

    #[test]
    fn rejects_unknown_pids() {
        assert!(counters_for(u32::MAX).is_err());
    }

    #[test]
    fn computing_totals_saturates() {
        let counters = ProcessIo {
            pid: 1,
            name: "demo".to_owned(),
            image: None,
            read_bytes: u64::MAX,
            write_bytes: 1,
            other_bytes: 1,
            read_operations: 0,
            write_operations: 0,
        };

        assert_eq!(counters.total_bytes(), u64::MAX);
    }
}
