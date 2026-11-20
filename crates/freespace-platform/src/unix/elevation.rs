//! Elevation state for Linux.
//!
//! There is no UAC prompt to trigger: a process either runs as root or it does
//! not. FreeSpace therefore reports the state and leaves the decision to the
//! user, who restarts it through `sudo` when an operation needs more rights.
//! Automatic re-launch is not offered because a graphical password agent is not
//! available in a portable, dependency-free package.

use std::{ffi::OsString, path::PathBuf};

use freespace_core::{Error, Result};
use rustix::process::geteuid;

use crate::ElevationOutcome;

/// Returns `true` when the effective user is root.
#[must_use]
pub fn is_elevated() -> bool {
    geteuid().as_raw() == 0
}

/// Returns `true` when the effective user is root.
///
/// Linux has no separate administrator group for elevation; root is the only
/// identity that passes every permission check.
#[must_use]
pub fn is_administrator() -> bool {
    is_elevated()
}

/// Automatic elevation is not possible without a polkit helper.
///
/// # Errors
/// Always returns [`Error::Unsupported`] with a hint to restart via `sudo`.
pub fn relaunch(
    _executable: &std::path::Path,
    _arguments: &[OsString],
) -> Result<ElevationOutcome> {
    Err(Error::Unsupported(
        "automatic elevation is not available on Linux; restart FreeSpace with `sudo`".to_owned(),
    ))
}

/// Returns the arguments that restart the current binary.
#[must_use]
pub fn relaunch_arguments() -> Vec<OsString> {
    std::env::args_os().skip(1).collect()
}

/// Returns the path of the running executable.
#[must_use]
pub fn current_executable() -> Option<PathBuf> {
    std::env::current_exe().ok()
}
