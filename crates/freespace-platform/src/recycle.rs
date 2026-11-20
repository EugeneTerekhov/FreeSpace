//! System trash facade.

use std::path::PathBuf;

use freespace_core::Result;

/// Moves `paths` to the system trash.
///
/// Returns the number of objects that disappeared from their original paths.
///
/// # Errors
/// Returns [`freespace_core::Error::InvalidArgument`] for an empty list and the
/// platform error when the trash is unavailable or the operation was aborted.
pub fn recycle(paths: &[PathBuf]) -> Result<u32> {
    crate::platform().trash.trash(paths)
}
