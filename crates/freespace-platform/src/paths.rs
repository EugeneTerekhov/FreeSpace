//! Path facade.
//!
//! Generic operations delegate to [`crate::platform`]; the Windows-only
//! verbatim helpers remain available on Windows until the migration to the
//! platform traits is complete.

use std::path::{Path, PathBuf};

use freespace_core::Result;

#[cfg(windows)]
pub use crate::windows::paths::{MAX_PATH_CHARS, is_verbatim, strip_verbatim, to_verbatim};

/// Resolves a path to the platform's canonical absolute form.
///
/// # Errors
/// Returns [`freespace_core::Error::InvalidPath`] for a path the platform
/// cannot represent.
pub fn normalize(path: &Path) -> Result<PathBuf> {
    crate::platform().paths.normalize(path)
}

/// Renders a path for logs and reports.
#[must_use]
pub fn display(path: &Path) -> String {
    crate::platform().paths.display(path)
}

/// Validates that a path fits the platform limit.
///
/// # Errors
/// Returns [`freespace_core::Error::InvalidPath`] when the path is too long.
pub fn validate_length(path: &Path) -> Result<()> {
    crate::platform().paths.validate_length(path)
}

/// Comparison key that follows the platform's case rules.
#[must_use]
pub fn equivalent_key(path: &Path) -> String {
    crate::platform().paths.equivalent_key(path)
}

/// Returns the root of the volume that contains `path`.
///
/// # Errors
/// Returns [`freespace_core::Error::InvalidPath`] when the path has no root.
pub fn volume_root(path: &Path) -> Result<PathBuf> {
    crate::platform().paths.volume_root(path)
}

/// Joins a file name and validates the resulting length before any change.
///
/// # Errors
/// Returns [`freespace_core::Error::InvalidArgument`] for an invalid name and
/// [`freespace_core::Error::InvalidPath`] when the result is too long.
pub fn join_within_limit(parent: &Path, name: &str) -> Result<PathBuf> {
    crate::platform().paths.join_within_limit(parent, name)
}

/// Verifies that `target` lies inside `root` (or is the root itself).
///
/// # Errors
/// Returns [`freespace_core::Error::Operation`] when the target escapes the
/// selected scope.
pub fn ensure_within(root: &Path, target: &Path) -> Result<()> {
    crate::platform().paths.ensure_within(root, target)
}
