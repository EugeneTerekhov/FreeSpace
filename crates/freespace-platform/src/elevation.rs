//! Elevation state and on-demand UAC relaunch (US21, FR-086…FR-088).
//!
//! FreeSpace never elevates itself silently: [`relaunch`] asks the shell to start
//! a second instance with the `runas` verb after the user explicitly requested
//! it, and reports a declined prompt as [`ElevationResult::Declined`] so the
//! caller can continue in limited mode.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use freespace_core::{Error, Result};
use windows::{
    Win32::{
        Foundation::{ERROR_CANCELLED, HANDLE},
        Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation},
        System::Threading::{GetCurrentProcess, OpenProcessToken},
        UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
    },
    core::PCWSTR,
};

use super::{
    command_line::quote_argument,
    win::{self, Handle},
};

/// Outcome of an elevation request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElevationResult {
    /// A second, elevated instance was started.
    Launched,
    /// The user dismissed the UAC prompt.
    Declined,
}

impl ElevationResult {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Launched => "launched",
            Self::Declined => "declined",
        }
    }
}

/// Returns `true` when the current process runs with an elevated token.
#[must_use]
pub fn is_elevated() -> bool {
    elevated_token().unwrap_or(false)
}

/// Returns `true` when the current process belongs to the Administrators group.
#[must_use]
pub fn is_administrator() -> bool {
    is_elevated()
}

fn elevated_token() -> Result<bool> {
    let mut token = HANDLE::default();
    // SAFETY: the pseudo-handle is valid for the process lifetime and the token
    // handle is owned after a successful call.
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut token) }
        .map_err(|error| Error::Io(win::win_error(&error)))?;
    let token = Handle::from_raw(token)?;
    let mut elevation = TOKEN_ELEVATION::default();
    let mut returned = 0u32;
    // SAFETY: `elevation` is writable and `returned` receives the used size.
    unsafe {
        GetTokenInformation(
            token.raw(),
            TokenElevation,
            Some(std::ptr::from_mut(&mut elevation).cast()),
            u32::try_from(size_of::<TOKEN_ELEVATION>()).unwrap_or(0),
            &mut returned,
        )
    }
    .map_err(|error| Error::Io(win::win_error(&error)))?;

    Ok(elevation.TokenIsElevated != 0)
}

/// Relaunches FreeSpace through the `runas` shell verb.
///
/// The prompt is the only UI shown by this function; the caller decides when it
/// is appropriate and how to explain the reason.
///
/// # Errors
/// Returns [`Error::InvalidArgument`] for a relative executable path and
/// [`Error::AccessDenied`] when the shell refuses to start the elevated process.
pub fn relaunch(executable: &Path, arguments: &[OsString]) -> Result<ElevationResult> {
    let command_line = command_line(executable, arguments)?;
    let executable_wide = win::to_wide(executable.as_os_str());
    let parameters = win::to_wide(std::ffi::OsStr::new(&command_line));
    let verb = win::to_wide(std::ffi::OsStr::new("runas"));

    // SAFETY: every pointer is a NUL-terminated wide string that lives for the
    // duration of the call; no output structures are requested.
    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(executable_wide.as_ptr()),
            PCWSTR(parameters.as_ptr()),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    let code = result.0 as isize;

    if code <= 32 {
        if code as u32 == ERROR_CANCELLED.0 {
            return Ok(ElevationResult::Declined);
        }

        return Err(Error::AccessDenied(format!(
            "the elevated launch failed with shell error {code}"
        )));
    }

    Ok(ElevationResult::Launched)
}

/// Builds the quoted parameter string passed to the elevated process.
///
/// # Errors
/// Returns [`Error::InvalidArgument`] when `executable` is relative.
pub fn command_line(executable: &Path, arguments: &[OsString]) -> Result<String> {
    if !executable.is_absolute() {
        return Err(Error::InvalidArgument(format!(
            "the elevated executable must be absolute: `{}`",
            executable.display()
        )));
    }

    let mut parts = Vec::with_capacity(arguments.len() + 1);
    parts.push(quote_argument(&executable.to_string_lossy()));
    parts.extend(
        arguments
            .iter()
            .map(|argument| quote_argument(&argument.to_string_lossy())),
    );

    Ok(parts.join(" "))
}

/// Arguments that restart the current binary with the same command line.
#[must_use]
pub fn relaunch_arguments() -> Vec<OsString> {
    std::env::args_os().skip(1).collect()
}

/// Path of the running executable, when it can be resolved.
#[must_use]
pub fn current_executable() -> Option<PathBuf> {
    std::env::current_exe().ok()
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::Path;

    use super::{ElevationResult, command_line, is_elevated, relaunch};

    #[test]
    fn reports_the_current_elevation_state() {
        let elevated = is_elevated();

        if elevated {
            assert!(super::is_administrator());
        }
    }

    #[test]
    fn quotes_every_argument() {
        let line = command_line(
            Path::new(r"C:\Program Files\FreeSpace\freespace-gui.exe"),
            &[
                OsString::from("--analyze"),
                OsString::from(r"D:\Мои документы\квартал 1"),
            ],
        )
        .expect("command line");

        assert!(line.starts_with('"'));
        assert!(line.contains("\"--analyze\"") || line.contains("--analyze"));
        assert!(line.contains("квартал 1\""));
    }

    #[test]
    fn rejects_relative_executables_without_prompting() {
        assert!(command_line(Path::new("freespace.exe"), &[]).is_err());
        assert!(
            relaunch(Path::new("freespace.exe"), &[]).is_err(),
            "no UAC prompt may appear for an invalid path"
        );
    }

    #[test]
    fn exposes_stable_result_names() {
        assert_eq!(ElevationResult::Launched.as_str(), "launched");
        assert_eq!(ElevationResult::Declined.as_str(), "declined");
    }
}
