//! Process I/O counters from `/proc/<pid>/io`.
//!
//! `/proc/<pid>/io` reports the bytes a process has read from and written to
//! storage (`read_bytes`, `write_bytes`) plus the operation counts (`syscr`,
//! `syscw`). Unlike ETW on Windows there is no event stream, so the existing
//! polling fallback in `freespace-scan` drives this backend. Parsing is kept in
//! separate helpers so it can be tested without a live `/proc`.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use freespace_core::{Error, Result};

use crate::common::monitor::ProcessIo;

/// Raw counters of one `/proc/<pid>/io` file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct IoCounters {
    /// Bytes fetched from storage.
    pub read_bytes: u64,
    /// Bytes sent to storage.
    pub write_bytes: u64,
    /// Read syscall count.
    pub read_operations: u64,
    /// Write syscall count.
    pub write_operations: u64,
}

/// Snapshots every process whose counters can be read.
///
/// Processes that vanish or deny access between the directory listing and the
/// read are skipped: a poll must not fail because one process exited.
///
/// # Errors
/// Returns [`Error::Io`] when `/proc` itself cannot be listed.
pub fn snapshot() -> Result<Vec<ProcessIo>> {
    let entries = fs::read_dir("/proc").map_err(Error::Io)?;
    let mut processes = Vec::new();

    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        if let Ok(process) = counters_for(pid) {
            processes.push(process);
        }
    }

    Ok(processes)
}

/// Reads the counters of one process.
///
/// # Errors
/// Returns [`Error::NotFound`] when the process no longer exists,
/// [`Error::AccessDenied`] when its counters are not readable and
/// [`Error::Operation`] for a malformed `/proc` record.
pub fn counters_for(pid: u32) -> Result<ProcessIo> {
    let io_path = PathBuf::from(format!("/proc/{pid}/io"));
    let text = fs::read_to_string(&io_path).map_err(|error| map_io(&io_path, error))?;
    let counters = parse_io(&text).ok_or_else(|| {
        Error::Operation(format!(
            "`{}` does not contain the expected counters",
            io_path.display()
        ))
    })?;

    Ok(ProcessIo {
        pid,
        name: process_name(pid),
        image: process_image(pid),
        read_bytes: counters.read_bytes,
        write_bytes: counters.write_bytes,
        other_bytes: 0,
        read_operations: counters.read_operations,
        write_operations: counters.write_operations,
    })
}

/// Parses the key/value lines of `/proc/<pid>/io`.
///
/// Returns `None` when one of the four counters is absent, which keeps a
/// truncated or permission-restricted read from being reported as zero usage.
pub(crate) fn parse_io(text: &str) -> Option<IoCounters> {
    let mut read_bytes = None;
    let mut write_bytes = None;
    let mut read_operations = None;
    let mut write_operations = None;

    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let Ok(value) = value.trim().parse::<u64>() else {
            continue;
        };
        match key.trim() {
            "read_bytes" => read_bytes = Some(value),
            "write_bytes" => write_bytes = Some(value),
            "syscr" => read_operations = Some(value),
            "syscw" => write_operations = Some(value),
            _ => {}
        }
    }

    Some(IoCounters {
        read_bytes: read_bytes?,
        write_bytes: write_bytes?,
        read_operations: read_operations?,
        write_operations: write_operations?,
    })
}

fn process_name(pid: u32) -> String {
    fs::read_to_string(format!("/proc/{pid}/comm"))
        .map(|name| name.trim_end().to_owned())
        .unwrap_or_default()
}

fn process_image(pid: u32) -> Option<PathBuf> {
    fs::read_link(format!("/proc/{pid}/exe")).ok()
}

fn map_io(path: &Path, error: io::Error) -> Error {
    match error.kind() {
        io::ErrorKind::NotFound => Error::NotFound(path.display().to_string()),
        io::ErrorKind::PermissionDenied => Error::AccessDenied(path.display().to_string()),
        _ => Error::Io(error),
    }
}

#[cfg(test)]
mod tests {
    use super::{counters_for, parse_io};

    const IO: &str = "\
rchar: 1024
wchar: 64
syscr: 12
syscw: 3
read_bytes: 4096
write_bytes: 8192
cancelled_write_bytes: 0
";

    #[test]
    fn parses_the_storage_counters() {
        let counters = parse_io(IO).expect("counters");

        assert_eq!(counters.read_bytes, 4_096);
        assert_eq!(counters.write_bytes, 8_192);
        assert_eq!(counters.read_operations, 12);
        assert_eq!(counters.write_operations, 3);
    }

    #[test]
    fn rejects_an_incomplete_record() {
        assert!(parse_io("rchar: 1\nwchar: 2\n").is_none());
    }

    #[test]
    fn reads_the_counters_of_the_current_process() {
        let process = counters_for(std::process::id()).expect("own counters");

        assert_eq!(process.pid, std::process::id());
        assert!(!process.name.is_empty());
        assert!(process.total_bytes() >= process.write_bytes);
    }

    #[test]
    fn reports_a_missing_process() {
        let error = counters_for(u32::MAX).expect_err("pid does not exist");

        assert_eq!(error.code(), freespace_core::ErrorCode::NotFound);
    }
}
