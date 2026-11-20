//! File metadata facade.
//!
//! Attribute and owner queries delegate to [`crate::platform`]. The Windows ACL
//! helpers are re-exported on Windows only; on other platforms the
//! corresponding capability is `false`.

use std::path::Path;

use freespace_core::Result;

#[cfg(windows)]
pub use crate::windows::security::{
    OwnerIdentity, copy_acl, last_error, owner_identity, reset_acl, set_owner,
};

/// Reads the platform attribute mask.
///
/// # Errors
/// Returns [`freespace_core::Error::NotFound`] for a missing object.
pub fn get_attributes(path: &Path) -> Result<u32> {
    crate::platform().metadata.attributes(path)
}

/// Applies `set` bits and clears `clear` bits.
///
/// # Errors
/// Returns an error when the attributes cannot be read or written.
pub fn set_attributes(path: &Path, set: u32, clear: u32) -> Result<u32> {
    crate::platform().metadata.set_attributes(path, set, clear)
}

/// Returns the owner name, or a platform-native identifier.
///
/// # Errors
/// Returns an error when the owner cannot be read.
pub fn owner(path: &Path) -> Result<Option<String>> {
    crate::platform().metadata.owner(path)
}
