//! Volume facade.

use std::path::Path;

use freespace_core::{Result, dashboard::Dashboard};

pub use crate::common::volumes::{DriveType, VolumeInfo};

/// Enumerates every mounted volume.
///
/// # Errors
/// Returns an error when the list of volumes cannot be obtained.
pub fn list_volumes() -> Result<Vec<VolumeInfo>> {
    crate::platform().volumes.list()
}

/// Builds the dashboard summary for every volume.
///
/// # Errors
/// Returns an error when the volume list cannot be obtained.
pub fn dashboard() -> Result<Dashboard> {
    let volumes = list_volumes()?
        .into_iter()
        .map(VolumeInfo::into_volume)
        .collect();

    Ok(Dashboard::from_volumes(volumes))
}

/// Queries the volume that contains `path`.
///
/// # Errors
/// Returns [`freespace_core::Error::InvalidPath`] when the path has no volume
/// root and an I/O error when the platform refuses the query.
pub fn describe_volume(path: &Path) -> Result<VolumeInfo> {
    crate::platform().volumes.describe(path)
}
