//! Restart Manager integration: who holds a file and how to release it.
//!
//! Closing another process' handle from user mode is impossible, so FreeSpace
//! follows the approach recommended by Microsoft: ask the Restart Manager to
//! list the applications that use the file, let it shut them down gracefully
//! (soft unlock) or terminate them explicitly after confirmation (force
//! unlock). Kernel-level locks fall back to the documented reboot queue
//! (`MOVEFILE_DELAY_UNTIL_REBOOT`).

use std::{
    ffi::OsStr,
    io,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
};

use freespace_core::{Error, Result};
use windows::{
    Win32::{
        Foundation::{ERROR_MORE_DATA, ERROR_SUCCESS, WIN32_ERROR},
        Storage::FileSystem::{
            CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_SHARE_MODE,
            MOVEFILE_DELAY_UNTIL_REBOOT, MoveFileExW, OPEN_EXISTING,
        },
        System::{
            RestartManager::{
                RM_PROCESS_INFO, RmEndSession, RmGetList, RmRegisterResources, RmShutdown,
                RmShutdownOnlyRegistered, RmStartSession,
            },
            Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess},
        },
    },
    core::PWSTR,
};

use super::win::{self, Handle};

/// Type of the application that holds a file.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApplicationType {
    /// Unknown application type.
    #[default]
    Unknown,
    /// Application with a main window.
    MainWindow,
    /// Application with another window.
    OtherWindow,
    /// Windows service.
    Service,
    /// Windows Explorer.
    Explorer,
    /// Console application.
    Console,
    /// Critical system process; must not be terminated.
    Critical,
}

impl ApplicationType {
    /// Maps the raw `RM_APP_TYPE` value.
    #[must_use]
    pub const fn from_raw(value: i32) -> Self {
        match value {
            1 => Self::MainWindow,
            2 => Self::OtherWindow,
            3 => Self::Service,
            4 => Self::Explorer,
            5 => Self::Console,
            1000 => Self::Critical,
            _ => Self::Unknown,
        }
    }

    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::MainWindow => "main-window",
            Self::OtherWindow => "other-window",
            Self::Service => "service",
            Self::Explorer => "explorer",
            Self::Console => "console",
            Self::Critical => "critical",
        }
    }

    /// Returns `true` when terminating the application is safe.
    #[must_use]
    pub const fn is_terminable(self) -> bool {
        !matches!(self, Self::Critical | Self::Unknown)
    }
}

/// One process that uses a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileLock {
    /// Process identifier.
    pub process_id: u32,
    /// Application name reported by the Restart Manager.
    pub process_name: String,
    /// Service short name, when the holder is a service.
    pub service_name: String,
    /// Application type.
    pub application_type: ApplicationType,
    /// Whether the manager can ask the application to close.
    pub can_soft_close: bool,
    /// Whether a reboot is required to release the object.
    pub requires_reboot: bool,
}

impl FileLock {
    /// Returns `true` when the process is critical and must be left alone.
    #[must_use]
    pub const fn is_critical(&self) -> bool {
        matches!(self.application_type, ApplicationType::Critical)
    }
}

/// A Restart Manager session that always ends on drop.
struct RmSession {
    handle: u32,
}

impl RmSession {
    fn start() -> Result<Self> {
        let mut handle = 0u32;
        let mut key = [0u16; 33];

        // SAFETY: the key buffer is writable and holds 33 UTF-16 units, which
        // is what RmStartSession expects (CCH_RM_SESSION_KEY + 1).
        let status = unsafe { RmStartSession(&mut handle, None, PWSTR(key.as_mut_ptr())) };
        check(status, "RmStartSession")?;

        Ok(Self { handle })
    }

    fn register(&self, path: &Path) -> Result<()> {
        let absolute = std::path::absolute(path).map_err(|error| Error::InvalidPath {
            path: path.to_path_buf(),
            reason: error.to_string(),
        })?;
        let mut wide: Vec<u16> = absolute.as_os_str().encode_wide().collect();
        wide.push(0);
        let filename = windows::core::PCWSTR(wide.as_ptr());
        let files = [filename];

        // SAFETY: `files` owns the only pointer and `wide` outlives the call.
        let status = unsafe { RmRegisterResources(self.handle, Some(&files), None, None) };
        check(status, "RmRegisterResources")
    }
}

impl Drop for RmSession {
    fn drop(&mut self) {
        // SAFETY: the session handle was created by RmStartSession and is
        // ended exactly once.
        unsafe {
            let _ = RmEndSession(self.handle);
        }
    }
}

fn check(status: WIN32_ERROR, operation: &str) -> Result<()> {
    if status == ERROR_SUCCESS {
        return Ok(());
    }

    tracing::debug!(operation, code = status.0, "Restart Manager call failed");

    Err(Error::Io(io::Error::from_raw_os_error(
        i32::try_from(status.0).unwrap_or(i32::MAX),
    )))
}

/// Lists the applications that currently hold `path`.
///
/// # Errors
/// Returns an error when the Restart Manager session cannot be created or the
/// object cannot be registered.
pub fn find_locks(path: &Path) -> Result<Vec<FileLock>> {
    let session = RmSession::start()?;
    session.register(path)?;

    let mut needed = 0u32;
    let mut count = 0u32;
    let mut reason = 0u32;

    // First call discovers how many records are required.
    // SAFETY: null output with zero capacity is the documented probe.
    let status = unsafe { RmGetList(session.handle, &mut needed, &mut count, None, &mut reason) };
    if status == ERROR_SUCCESS && needed == 0 {
        return Ok(Vec::new());
    }
    if status != ERROR_MORE_DATA {
        check(status, "RmGetList")?;
    }

    let mut records = vec![RM_PROCESS_INFO::default(); needed.max(1) as usize];
    count = needed;
    // SAFETY: the vector holds exactly `count` records.
    let status = unsafe {
        RmGetList(
            session.handle,
            &mut needed,
            &mut count,
            Some(records.as_mut_ptr()),
            &mut reason,
        )
    };
    check(status, "RmGetList")?;

    let requires_reboot = reason != 0;
    let records = &records[..count as usize];
    let locks = records
        .iter()
        .map(|record| {
            let application_type = ApplicationType::from_raw(record.ApplicationType.0);
            FileLock {
                process_id: record.Process.dwProcessId,
                process_name: utf16_string(&record.strAppName),
                service_name: utf16_string(&record.strServiceShortName),
                application_type,
                can_soft_close: !matches!(application_type, ApplicationType::Critical),
                requires_reboot,
            }
        })
        .collect();

    Ok(locks)
}

/// Asks the Restart Manager to shut the registered applications down.
///
/// Only applications that registered themselves for restart are asked, so the
/// operation is as gentle as the platform allows.
///
/// # Errors
/// Returns an error when the session cannot be created or the shutdown fails.
pub fn release_soft(path: &Path) -> Result<u32> {
    let locked = find_locks(path)?.len();
    let session = RmSession::start()?;
    session.register(path)?;

    // SAFETY: the handle belongs to this session.
    let status = unsafe {
        RmShutdown(
            session.handle,
            u32::try_from(RmShutdownOnlyRegistered.0).unwrap_or(0),
            None,
        )
    };
    check(status, "RmShutdown")?;

    Ok(u32::try_from(locked).unwrap_or(u32::MAX))
}

/// Terminates the process with the given identifier.
///
/// Callers must have collected explicit confirmation first; the GUI and the
/// CLI enforce that before reaching this function (FR-022).
///
/// # Errors
/// Returns [`Error::AccessDenied`] when the process cannot be opened and
/// [`Error::Io`] when termination fails.
pub fn terminate_process(process_id: u32) -> Result<()> {
    if process_id == 0 || process_id == std::process::id() {
        return Err(Error::InvalidArgument(
            "refusing to terminate the current process".to_owned(),
        ));
    }

    // SAFETY: the call only opens a handle; the value is closed by `Handle`.
    let handle = unsafe { OpenProcess(PROCESS_TERMINATE, false, process_id) }
        .map_err(|error| Error::Io(win::win_error(&error)))?;
    let handle = Handle::from_raw(handle)?;

    // SAFETY: the handle has PROCESS_TERMINATE access and is owned here.
    unsafe { TerminateProcess(handle.raw(), 1) }.map_err(|error| Error::Io(win::win_error(&error)))
}

/// Schedules deletion of `path` before the next graphical logon.
///
/// # Errors
/// Returns [`Error::AccessDenied`] when the pending-rename key cannot be
/// written, which is typical without elevation.
pub fn schedule_delete_on_reboot(path: &Path) -> Result<()> {
    let verbatim = super::paths::to_verbatim(path)?;
    let wide = win::to_wide(verbatim.as_os_str());

    // SAFETY: `wide` is NUL-terminated and lives for the call.
    unsafe {
        MoveFileExW(
            windows::core::PCWSTR(wide.as_ptr()),
            windows::core::PCWSTR::null(),
            MOVEFILE_DELAY_UNTIL_REBOOT,
        )
    }
    .map_err(|error| Error::Io(win::win_error(&error)))
}

/// Returns `true` when the current user can open `path` without sharing.
///
/// Sharing or lock violations mean another process still holds the object.
///
/// # Errors
/// Returns [`Error::InvalidPath`] when the path is unusable. Missing objects
/// are reported as unavailable instead of as an error.
pub fn is_available(path: &Path) -> Result<bool> {
    let verbatim = super::paths::to_verbatim(path)?;
    let wide = win::to_wide(verbatim.as_os_str());

    // SAFETY: the path buffer is NUL-terminated and lives for the call.
    let result = unsafe {
        CreateFileW(
            windows::core::PCWSTR(wide.as_ptr()),
            FILE_GENERIC_READ.0,
            FILE_SHARE_MODE(0),
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    };

    match result {
        Ok(handle) => {
            let handle = Handle::from_raw(handle)?;
            drop(handle);
            Ok(true)
        }
        Err(error) => {
            let io = win::win_error(&error);
            match io.raw_os_error() {
                // ERROR_SHARING_VIOLATION, ERROR_LOCK_VIOLATION, ERROR_ACCESS_DENIED
                Some(32 | 33 | 5) => Ok(false),
                // ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND
                Some(2 | 3) => Ok(false),
                _ => Err(Error::Io(io)),
            }
        }
    }
}

/// Returns the absolute path passed to the Restart Manager.
#[must_use]
pub fn describe_path(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

fn utf16_string(units: &[u16]) -> String {
    let end = units
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(units.len());

    String::from_utf16_lossy(&units[..end])
}

/// Builds a NUL-terminated UTF-16 copy of an `OsStr` for callers that need it.
#[must_use]
pub fn wide(value: &OsStr) -> Vec<u16> {
    let mut units: Vec<u16> = value.encode_wide().collect();
    units.push(0);

    units
}

#[cfg(test)]
mod tests {
    use super::{ApplicationType, FileLock, find_locks, is_available, utf16_string};

    #[test]
    fn maps_application_types() {
        assert_eq!(ApplicationType::from_raw(1), ApplicationType::MainWindow);
        assert_eq!(ApplicationType::from_raw(3), ApplicationType::Service);
        assert_eq!(ApplicationType::from_raw(1000), ApplicationType::Critical);
        assert_eq!(ApplicationType::from_raw(99), ApplicationType::Unknown);
        assert!(!ApplicationType::Critical.is_terminable());
        assert!(ApplicationType::MainWindow.is_terminable());
    }

    #[test]
    fn decodes_process_names() {
        let units = [
            b'n' as u16,
            b'o' as u16,
            b't' as u16,
            b'e' as u16,
            0,
            b'x' as u16,
        ];

        assert_eq!(utf16_string(&units), "note");
    }

    #[test]
    fn finds_no_locks_for_an_unused_temporary_file() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("free.txt");
        std::fs::write(&path, b"payload").expect("write file");

        let locks = find_locks(&path).expect("query locks");

        assert!(
            locks
                .iter()
                .all(|lock| lock.process_id != std::process::id()),
            "the query itself must not register as a lock holder"
        );
        assert!(is_available(&path).expect("availability"));
        let _ = FileLock {
            process_id: 1,
            process_name: "app".to_owned(),
            service_name: String::new(),
            application_type: ApplicationType::MainWindow,
            can_soft_close: true,
            requires_reboot: false,
        };
    }

    #[test]
    fn reports_missing_files_as_unavailable() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("missing.txt");

        assert!(!is_available(&path).expect("availability"));
    }
}
