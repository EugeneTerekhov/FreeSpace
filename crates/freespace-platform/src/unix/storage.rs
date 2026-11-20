//! Storage media hints from sysfs.
//!
//! The kernel exposes whether a block device rotates in
//! `/sys/dev/block/<major>:<minor>/queue/rotational`. A partition's sysfs node
//! is a symlink whose parent holds the queue attributes, so the lookup falls
//! back to the parent when the direct path is absent. TRIM belongs to the
//! shredder and is therefore unavailable on Linux.

use std::{
    fs,
    os::unix::fs::MetadataExt as _,
    path::{Path, PathBuf},
};

use freespace_core::{Error, Result};
use rustix::fs::{major, minor};

use crate::common::storage::StorageKind;

/// Detects the media kind of the volume that contains `path`.
///
/// Returns [`StorageKind::Unknown`] for pseudo devices (tmpfs, overlay) and for
/// paths whose metadata cannot be read; the caller treats it as advisory.
///
/// # Errors
/// Currently never fails; the signature matches [`crate::StorageHints`].
pub fn kind(path: &Path) -> Result<StorageKind> {
    let Ok(metadata) = fs::metadata(path) else {
        return Ok(StorageKind::Unknown);
    };
    let device = metadata.dev();
    let major = major(device);
    let minor = minor(device);

    Ok(rotational_for(major, minor)
        .map(|value| kind_from_rotational(&value))
        .unwrap_or(StorageKind::Unknown))
}

/// Asks the file system to discard the clusters of a file; unavailable.
///
/// # Errors
/// Always returns [`Error::Unsupported`].
pub fn trim(_path: &Path) -> Result<()> {
    Err(Error::Unsupported(
        "TRIM is only used by the Windows shredder".to_owned(),
    ))
}

/// Reads `queue/rotational` for a block device.
///
/// Partitions do not own a queue directory, so the parent of the resolved sysfs
/// entry is checked as well.
pub(crate) fn rotational_for(major: u32, minor: u32) -> Option<String> {
    let base = PathBuf::from(format!("/sys/dev/block/{major}:{minor}"));
    let candidates = [
        base.join("queue/rotational"),
        base.join("../queue/rotational"),
    ];

    candidates.iter().find_map(|candidate| {
        fs::read_to_string(candidate)
            .ok()
            .map(|value| value.trim().to_owned())
    })
}

/// Maps the sysfs `rotational` flag to a media kind.
pub(crate) fn kind_from_rotational(value: &str) -> StorageKind {
    match value.trim() {
        "0" => StorageKind::Ssd,
        "1" => StorageKind::Hdd,
        _ => StorageKind::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::{kind_from_rotational, rotational_for};
    use crate::common::storage::StorageKind;

    #[test]
    fn maps_the_rotational_flag() {
        assert_eq!(kind_from_rotational("0"), StorageKind::Ssd);
        assert_eq!(kind_from_rotational("1"), StorageKind::Hdd);
        assert_eq!(kind_from_rotational("0\n"), StorageKind::Ssd);
        assert_eq!(kind_from_rotational(""), StorageKind::Unknown);
        assert_eq!(kind_from_rotational("maybe"), StorageKind::Unknown);
    }

    #[test]
    fn unknown_devices_have_no_rotational_record() {
        // 0:0 is never a real block device.
        assert!(rotational_for(0, 0).is_none());
    }
}
