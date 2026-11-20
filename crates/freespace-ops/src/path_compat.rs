//! Cross-platform access to the verbatim path helpers.
//!
//! On Windows the platform crate owns the `\\?\` conversions. On Linux there is
//! no verbatim prefix, so the same helpers resolve the path to its absolute
//! form (or return it unchanged for display). Keeping the wrapper here lets the
//! rest of `freespace-ops` stay free of `cfg(windows)` blocks.

#[cfg(not(windows))]
use std::path::{Path, PathBuf};

#[cfg(not(windows))]
use freespace_core::Result;

#[cfg(windows)]
pub use freespace_platform::paths::{strip_verbatim, to_verbatim};

/// Resolves `path` to the native absolute form used for file-system calls.
///
/// # Errors
/// Returns [`freespace_core::Error::InvalidPath`] when the path is too long or
/// cannot be made absolute.
#[cfg(not(windows))]
pub fn to_verbatim(path: &Path) -> Result<PathBuf> {
    freespace_platform::paths::normalize(path)
}

/// Removes the platform's verbatim prefix.
#[cfg(not(windows))]
#[must_use]
pub fn strip_verbatim(path: &Path) -> PathBuf {
    path.to_path_buf()
}
