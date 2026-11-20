//! Volume enumeration through `GetLogicalDrives` and `GetVolumeInformationW`.

use std::{
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
};

use freespace_core::{
    Error, Result,
    model::{FileSystem, VolumeStatus},
};

pub use crate::common::volumes::{DriveType, VolumeInfo};
use windows::{
    Win32::Storage::FileSystem::{
        GetDiskFreeSpaceExW, GetDiskFreeSpaceW, GetDriveTypeW, GetLogicalDrives,
        GetVolumeInformationW,
    },
    core::PCWSTR,
};

use super::{paths, win};

const MAX_VOLUME_LABEL: usize = 261;
const MAX_FILE_SYSTEM_NAME: usize = 32;

/// Enumerates every logical drive, including volumes that cannot be queried.
///
/// Drives that fail the query (empty card readers, disconnected network shares)
/// are still reported with [`VolumeStatus::Inaccessible`] so the dashboard can
/// show them instead of silently dropping them.
///
/// # Errors
/// Returns an error only when the drive bit mask itself cannot be obtained.
pub fn list_volumes() -> Result<Vec<VolumeInfo>> {
    // SAFETY: GetLogicalDrives has no preconditions.
    let mask = unsafe { GetLogicalDrives() };
    if mask == 0 {
        return Err(Error::Io(win::last_error()));
    }

    let mut volumes = Vec::new();
    for index in 0..26u8 {
        if mask & (1 << index) == 0 {
            continue;
        }

        let letter = char::from(b'A' + index);
        let mount_point = PathBuf::from(format!("{letter}:\\"));

        match query_volume(&mount_point) {
            Ok(info) => volumes.push(info),
            Err(error) => {
                tracing::debug!(
                    volume = %mount_point.display(),
                    %error,
                    "volume query failed"
                );
                volumes.push(VolumeInfo {
                    mount_point,
                    label: String::new(),
                    file_system: FileSystem::Other,
                    total_bytes: 0,
                    free_bytes: 0,
                    cluster_size: None,
                    serial_number: 0,
                    drive_type: DriveType::Unknown,
                    status: VolumeStatus::Inaccessible,
                });
            }
        }
    }

    Ok(volumes)
}

/// Queries the volume that contains `path`.
///
/// # Errors
/// Returns [`Error::InvalidPath`] when the path has no volume root and
/// [`Error::Io`] when Windows refuses the query.
pub fn describe_volume(path: &Path) -> Result<VolumeInfo> {
    let root = paths::volume_root(path)?;

    query_volume(&root)
}

fn query_volume(mount_point: &Path) -> Result<VolumeInfo> {
    let mut root_units: Vec<u16> = mount_point.as_os_str().encode_wide().collect();
    if root_units.last() != Some(&(b'\\' as u16)) {
        root_units.push(b'\\' as u16);
    }
    root_units.push(0);
    let root_ptr = PCWSTR(root_units.as_ptr());

    let mut label = [0u16; MAX_VOLUME_LABEL];
    let mut file_system = [0u16; MAX_FILE_SYSTEM_NAME];
    let mut serial_number = 0u32;
    let mut maximum_component = 0u32;
    let mut file_system_flags = 0u32;

    // SAFETY: buffers live for the duration of the call and are NUL-terminated
    // by Windows, which respects the supplied lengths.
    unsafe {
        GetVolumeInformationW(
            root_ptr,
            Some(&mut label),
            Some(&mut serial_number),
            Some(&mut maximum_component),
            Some(&mut file_system_flags),
            Some(&mut file_system),
        )
    }
    .map_err(|error| Error::Io(win::win_error(&error)))?;

    let mut free_to_caller = 0u64;
    let mut total_bytes = 0u64;
    let mut total_free = 0u64;

    // SAFETY: the root pointer is valid for the duration of the call.
    unsafe {
        GetDiskFreeSpaceExW(
            root_ptr,
            Some(&mut free_to_caller),
            Some(&mut total_bytes),
            Some(&mut total_free),
        )
    }
    .map_err(|error| Error::Io(win::win_error(&error)))?;

    let free_bytes = if free_to_caller == 0 {
        total_free
    } else {
        free_to_caller
    };

    let mut sectors_per_cluster = 0u32;
    let mut bytes_per_sector = 0u32;
    let mut free_clusters = 0u32;
    let mut total_clusters = 0u32;

    // SAFETY: the root pointer is valid for the duration of the call.
    let cluster_size = unsafe {
        GetDiskFreeSpaceW(
            root_ptr,
            Some(&mut sectors_per_cluster),
            Some(&mut bytes_per_sector),
            Some(&mut free_clusters),
            Some(&mut total_clusters),
        )
    }
    .ok()
    .and_then(|()| sectors_per_cluster.checked_mul(bytes_per_sector))
    .filter(|size| *size > 0);

    // SAFETY: GetDriveTypeW only inspects the supplied NUL-terminated root.
    let drive_type = DriveType::from_raw(unsafe { GetDriveTypeW(root_ptr) });

    let label = win::from_wide(&label).to_string_lossy().into_owned();
    let file_system_name = win::from_wide(&file_system).to_string_lossy().into_owned();

    let info = VolumeInfo {
        mount_point: mount_point.to_path_buf(),
        label,
        file_system: FileSystem::from_name(&file_system_name),
        total_bytes,
        free_bytes,
        cluster_size,
        serial_number,
        drive_type,
        status: VolumeStatus::Available,
    };

    if info.free_bytes > info.total_bytes {
        return Err(Error::Operation(format!(
            "volume `{}` reports more free than total space",
            info.mount_point.display()
        )));
    }

    Ok(info)
}

#[cfg(test)]
fn mount_point_ends_with_separator(volume: &VolumeInfo) -> bool {
    volume
        .mount_point
        .as_os_str()
        .to_string_lossy()
        .ends_with('\\')
}

#[cfg(test)]
mod tests {
    use super::{DriveType, describe_volume, list_volumes, mount_point_ends_with_separator};
    use freespace_core::model::VolumeStatus;

    #[test]
    fn enumerates_logical_drives() {
        let volumes = list_volumes().expect("enumerate volumes");

        assert!(
            !volumes.is_empty(),
            "a Windows machine has at least one drive"
        );
        for volume in &volumes {
            let domain = volume.clone().into_volume();

            assert!(domain.validate().is_ok(), "invalid volume: {domain:?}");
            assert!(mount_point_ends_with_separator(volume));
        }
    }

    #[test]
    fn describes_the_current_volume() {
        let directory = tempfile::tempdir().expect("temp dir");
        let info = describe_volume(directory.path()).expect("describe volume");

        assert_eq!(info.status, VolumeStatus::Available);
        assert!(info.total_bytes > 0);
        assert!(info.free_bytes <= info.total_bytes);

        let domain = info.clone().into_volume();
        assert_eq!(domain.mount_point, info.mount_point);
        assert_eq!(
            domain.id,
            freespace_core::model::Volume::id_for_mount_point(&info.mount_point)
        );
    }

    #[test]
    fn maps_drive_types() {
        assert_eq!(DriveType::from_raw(2), DriveType::Removable);
        assert_eq!(DriveType::from_raw(3), DriveType::Fixed);
        assert_eq!(DriveType::from_raw(99), DriveType::Unknown);
        assert!(DriveType::Removable.is_removable());
        assert!(!DriveType::Fixed.is_removable());
    }

    #[test]
    fn builds_a_dashboard_summary() {
        let dashboard = crate::volumes::dashboard().expect("dashboard");

        assert!(!dashboard.volumes.is_empty());
        assert!(dashboard.totals().volumes >= dashboard.totals().available);
        for summary in &dashboard.volumes {
            assert!(summary.volume.free_bytes <= summary.volume.total_bytes);
            assert!(summary.used_percent <= 100);
        }
    }
}
