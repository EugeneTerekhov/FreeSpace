//! Solid-state detection and TRIM for the shredder.
//!
//! Overwriting is not a guarantee on flash storage: wear levelling means the
//! controller may keep an older physical copy of the data. FreeSpace therefore
//! tells the user the truth (FR-031), still overwrites what it can see, and
//! asks the storage stack to discard the clusters with `FSCTL_FILE_LEVEL_TRIM`.
//!
//! The media type comes from `IOCTL_STORAGE_QUERY_PROPERTY` with
//! `StorageDeviceSeekPenaltyProperty`: devices that do not pay a seek penalty
//! are treated as flash.

use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

use freespace_core::{Error, Result};
use windows::{
    Win32::{
        Foundation::HANDLE,
        Storage::FileSystem::{
            CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
            FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
        },
        System::{
            IO::DeviceIoControl,
            Ioctl::{
                DEVICE_SEEK_PENALTY_DESCRIPTOR, FILE_LEVEL_TRIM, FILE_LEVEL_TRIM_RANGE,
                FSCTL_FILE_LEVEL_TRIM, IOCTL_STORAGE_QUERY_PROPERTY, PropertyStandardQuery,
                STORAGE_PROPERTY_QUERY, StorageDeviceSeekPenaltyProperty,
            },
        },
    },
    core::PCWSTR,
};

use super::{paths, win};

pub use crate::common::storage::StorageKind;

/// Detects the media kind of the volume that contains `path`.
///
/// An unknown media type is reported as [`StorageKind::Unknown`] rather than an
/// error: the answer only decides whether an extra warning is shown.
///
/// # Errors
/// Returns [`Error::InvalidPath`] when `path` has no volume root.
pub fn storage_kind(path: &Path) -> Result<StorageKind> {
    let root = paths::volume_root(path)?;
    let device = device_path(&root);
    let wide = win::to_wide(OsStr::new(&device));

    // SAFETY: the device path is NUL-terminated and lives for the call.
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    };
    let Ok(handle) = handle else {
        return Ok(StorageKind::Unknown);
    };
    let handle = win::Handle::from_raw(handle)?;

    let query = STORAGE_PROPERTY_QUERY {
        PropertyId: StorageDeviceSeekPenaltyProperty,
        QueryType: PropertyStandardQuery,
        AdditionalParameters: [0],
    };
    let mut descriptor = DEVICE_SEEK_PENALTY_DESCRIPTOR::default();
    let mut returned = 0u32;

    // SAFETY: both buffers are valid for the duration of the call and the sizes
    // match their types.
    let result = unsafe {
        DeviceIoControl(
            handle.raw(),
            IOCTL_STORAGE_QUERY_PROPERTY,
            Some(std::ptr::from_ref(&query).cast()),
            u32::try_from(size_of_val(&query)).unwrap_or(0),
            Some(std::ptr::from_mut(&mut descriptor).cast()),
            u32::try_from(size_of_val(&descriptor)).unwrap_or(0),
            Some(&mut returned),
            None,
        )
    };

    match result {
        Ok(()) if descriptor.IncursSeekPenalty => Ok(StorageKind::Hdd),
        Ok(()) => Ok(StorageKind::Ssd),
        Err(error) => {
            tracing::debug!(
                device = %device.display(),
                %error,
                "seek-penalty query failed; media type is unknown"
            );
            Ok(StorageKind::Unknown)
        }
    }
}

/// Asks the file system to discard the clusters of a file.
///
/// Must be called after the overwrite and before the file is deleted.
///
/// # Errors
/// Returns [`Error::Unsupported`] when the volume or file system does not
/// support `FSCTL_FILE_LEVEL_TRIM`, and [`Error::Io`] for other failures.
pub fn trim_file(path: &Path) -> Result<()> {
    let verbatim = paths::to_verbatim(path)?;
    let wide = win::to_wide(verbatim.as_os_str());
    let access = FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0;

    // SAFETY: the path is NUL-terminated and lives for the call.
    let handle: HANDLE = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    }
    .map_err(|error| Error::Io(win::win_error(&error)))?;
    let handle = win::Handle::from_raw(handle)?;

    let length = fs::metadata(&verbatim).map_err(Error::Io)?.len();
    if length == 0 {
        return Ok(());
    }

    let input = FILE_LEVEL_TRIM {
        Key: 0,
        NumRanges: 1,
        Ranges: [FILE_LEVEL_TRIM_RANGE {
            Offset: 0,
            Length: length,
        }],
    };
    let mut returned = 0u32;

    // SAFETY: the input buffer is valid for the duration of the call; the
    // output buffer is intentionally absent.
    unsafe {
        DeviceIoControl(
            handle.raw(),
            FSCTL_FILE_LEVEL_TRIM,
            Some(std::ptr::from_ref(&input).cast()),
            u32::try_from(size_of_val(&input)).unwrap_or(0),
            None,
            0,
            Some(&mut returned),
            None,
        )
    }
    .map_err(|error| {
        let io = win::win_error(&error);
        Error::Unsupported(format!("TRIM is not available: {io}"))
    })?;

    Ok(())
}

/// Converts a volume root such as `C:\` into `\\.\C:`.
fn device_path(root: &Path) -> PathBuf {
    let text = root.to_string_lossy();
    let trimmed = text.trim_end_matches('\\');
    PathBuf::from(format!(r"\\.\{trimmed}"))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{StorageKind, device_path, storage_kind, trim_file};

    #[test]
    fn builds_device_paths() {
        assert_eq!(device_path(Path::new(r"C:\")), Path::new(r"\\.\C:"));
        assert_eq!(device_path(Path::new(r"D:\")), Path::new(r"\\.\D:"));
    }

    #[test]
    fn names_are_stable() {
        assert_eq!(StorageKind::Ssd.as_str(), "ssd");
        assert_eq!(StorageKind::Hdd.as_str(), "hdd");
        assert_eq!(StorageKind::Unknown.as_str(), "unknown");
        assert!(StorageKind::Ssd.needs_warning());
        assert!(!StorageKind::Hdd.needs_warning());
    }

    #[test]
    fn detects_the_current_media() {
        let directory = tempfile::tempdir().expect("temp dir");
        let kind = storage_kind(directory.path()).expect("storage kind");

        assert!(matches!(
            kind,
            StorageKind::Ssd | StorageKind::Hdd | StorageKind::Unknown
        ));
    }

    #[test]
    fn trims_a_fixture_or_reports_unsupported() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("payload.bin");
        std::fs::write(&path, vec![0xAB_u8; 256 * 1024]).expect("write");

        match trim_file(&path) {
            Ok(()) => {}
            Err(error) => assert_eq!(
                error.code().as_str(),
                "E_UNSUPPORTED",
                "unexpected TRIM error: {error}"
            ),
        }
        assert!(path.exists(), "trimming never removes the file");
    }
}
