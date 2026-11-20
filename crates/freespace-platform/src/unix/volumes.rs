//! Volume enumeration from `/proc/self/mounts` and `statvfs`.
//!
//! The kernel mount table gives mount points and file-system names; `statvfs`
//! gives the block counts. Pseudo file systems (`proc`, `sysfs`, …) carry no
//! useful capacity and are filtered out. `tmpfs` is kept: it reports real block
//! counts and users scan `/tmp` or `/dev/shm` deliberately.

use std::path::{Path, PathBuf};

use freespace_core::{
    Result,
    model::{FileSystem, VolumeStatus},
};
use rustix::fs::StatVfs;

use super::paths::{self, MountEntry};
use crate::common::volumes::{DriveType, VolumeInfo};

/// File systems that describe kernel state rather than storage.
const PSEUDO_FILE_SYSTEMS: &[&str] = &[
    "proc",
    "sysfs",
    "devtmpfs",
    "devpts",
    "rootfs",
    "ramfs",
    "cgroup",
    "cgroup2",
    "pstore",
    "securityfs",
    "debugfs",
    "tracefs",
    "configfs",
    "fusectl",
    "mqueue",
    "hugetlbfs",
    "autofs",
    "binfmt_misc",
    "rpc_pipefs",
    "selinuxfs",
    "efivarfs",
    "bpf",
    "nsfs",
    "squashfs",
    "fuse.gvfsd-fuse",
    "fuse.portal",
    "fuse.lxcfs",
];

/// Lists every mounted volume on the local machine.
///
/// Mounts are de-duplicated by mount point. A mount whose `statvfs` call fails
/// is skipped: it is either gone already or not readable as a volume.
///
/// # Errors
/// Currently never fails; the signature matches [`crate::Volumes`] so a future
/// kernel interface can report errors without changing callers.
pub fn list() -> Result<Vec<VolumeInfo>> {
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut volumes = Vec::new();

    for entry in paths::read_mounts() {
        if is_pseudo(&entry.file_system) || seen.contains(&entry.mount_point) {
            continue;
        }
        let Ok(stat) = rustix::fs::statvfs(&entry.mount_point) else {
            continue;
        };
        seen.push(entry.mount_point.clone());
        volumes.push(volume_info(
            &entry,
            StatValues::from_stat(&stat),
            VolumeStatus::Available,
        ));
    }

    Ok(volumes)
}

/// Describes the volume that contains `path`.
///
/// # Errors
/// Returns [`Error::InvalidPath`] when `path` cannot be made absolute. A volume
/// whose statistics cannot be read is returned with zero sizes and
/// [`VolumeStatus::Inaccessible`] instead of failing the call.
pub fn describe(path: &Path) -> Result<VolumeInfo> {
    let normalized = paths::normalize(path)?;
    let entries = paths::read_mounts();
    let fallback = MountEntry {
        source: String::new(),
        mount_point: PathBuf::from("/"),
        file_system: String::new(),
    };
    let entry = paths::mount_entry_for(&entries, &normalized).unwrap_or(&fallback);

    let values =
        rustix::fs::statvfs(&entry.mount_point).or_else(|_| rustix::fs::statvfs(&normalized));

    Ok(match values {
        Ok(stat) => volume_info(entry, StatValues::from_stat(&stat), VolumeStatus::Available),
        Err(_) => volume_info(entry, StatValues::empty(), VolumeStatus::Inaccessible),
    })
}

/// Returns `true` for file systems without useful storage statistics.
pub(crate) fn is_pseudo(file_system: &str) -> bool {
    let name = file_system.to_ascii_lowercase();

    PSEUDO_FILE_SYSTEMS.contains(&name.as_str())
}

/// Block counts of one mount.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StatValues {
    /// Total size in bytes.
    pub total_bytes: u64,
    /// Space available to the current user.
    pub free_bytes: u64,
    /// Allocation unit in bytes.
    pub cluster_size: Option<u32>,
}

impl StatValues {
    fn from_stat(stat: &StatVfs) -> Self {
        let unit = if stat.f_frsize > 0 {
            stat.f_frsize
        } else {
            stat.f_bsize
        };

        Self {
            total_bytes: stat.f_blocks.saturating_mul(unit),
            free_bytes: stat.f_bavail.saturating_mul(unit),
            cluster_size: u32::try_from(unit).ok().filter(|value| *value > 0),
        }
    }

    const fn empty() -> Self {
        Self {
            total_bytes: 0,
            free_bytes: 0,
            cluster_size: None,
        }
    }
}

/// Builds a domain record from one mount entry and its statistics.
pub(crate) fn volume_info(
    entry: &MountEntry,
    values: StatValues,
    status: VolumeStatus,
) -> VolumeInfo {
    VolumeInfo {
        mount_point: entry.mount_point.clone(),
        label: label_for(&entry.source),
        file_system: FileSystem::from_name(&entry.file_system),
        total_bytes: values.total_bytes,
        free_bytes: values.free_bytes,
        cluster_size: values.cluster_size,
        serial_number: stable_serial(&entry.source),
        drive_type: drive_type(entry),
        status,
    }
}

/// Classifies the media behind a mount.
pub(crate) fn drive_type(entry: &MountEntry) -> DriveType {
    let file_system = entry.file_system.to_ascii_lowercase();
    let remote = matches!(
        file_system.as_str(),
        "nfs" | "nfs4" | "cifs" | "smb3" | "sshfs" | "fuse.sshfs" | "9p" | "ceph"
    ) || (file_system.starts_with("fuse.")
        && ["ssh", "rclone", "smb", "nfs", "davfs"]
            .iter()
            .any(|marker| file_system.contains(marker)));

    if remote {
        return DriveType::Remote;
    }

    if ["/media", "/run/media", "/mnt"]
        .iter()
        .any(|root| entry.mount_point.starts_with(root))
    {
        return DriveType::Removable;
    }

    DriveType::Fixed
}

/// Reads the label of the device from `/dev/disk/by-label`.
///
/// Returns an empty string when the device has no label or the udev symlinks
/// are unavailable; labels are decoration, not identity.
pub(crate) fn label_for(source: &str) -> String {
    if source.is_empty() || !source.starts_with('/') {
        return String::new();
    }
    let Ok(source_path) = std::fs::canonicalize(source) else {
        return String::new();
    };
    let Ok(entries) = std::fs::read_dir("/dev/disk/by-label") else {
        return String::new();
    };

    for entry in entries.flatten() {
        if std::fs::canonicalize(entry.path()).is_ok_and(|target| target == source_path) {
            return decode_udev_escape(&entry.file_name().to_string_lossy());
        }
    }

    String::new()
}

/// Decodes the `\xNN` escapes udev uses in by-label names.
pub(crate) fn decode_udev_escape(name: &str) -> String {
    let raw = name.as_bytes();
    let mut decoded = Vec::with_capacity(raw.len());
    let mut index = 0;

    while index < raw.len() {
        if raw[index] == b'\\' && index + 3 < raw.len() && raw[index + 1] == b'x' {
            if let Ok(value) = u8::from_str_radix(&name[index + 2..index + 4], 16) {
                decoded.push(value);
                index += 4;
                continue;
            }
        }
        decoded.push(raw[index]);
        index += 1;
    }

    String::from_utf8_lossy(&decoded).into_owned()
}

/// Stable 32-bit device identifier (FNV-1a over the mount source).
pub(crate) fn stable_serial(source: &str) -> u32 {
    let mut hash = 0x811c_9dc5_u32;

    for byte in source.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }

    hash
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use freespace_core::model::{FileSystem, VolumeStatus};

    use super::{
        StatValues, decode_udev_escape, drive_type, is_pseudo, stable_serial, volume_info,
    };
    use crate::common::volumes::DriveType;
    use crate::unix::paths::{MountEntry, parse_mounts};

    fn entry(mount_point: &str, file_system: &str, source: &str) -> MountEntry {
        MountEntry {
            source: source.to_owned(),
            mount_point: PathBuf::from(mount_point),
            file_system: file_system.to_owned(),
        }
    }

    #[test]
    fn builds_a_volume_from_a_mount_and_statvfs_values() {
        let mount = entry("/mnt/data", "ext4", "fixture-device");
        let values = StatValues {
            total_bytes: 1_000,
            free_bytes: 400,
            cluster_size: Some(4_096),
        };

        let volume = volume_info(&mount, values, VolumeStatus::Available);

        assert_eq!(volume.mount_point, PathBuf::from("/mnt/data"));
        assert_eq!(volume.file_system, FileSystem::Other);
        assert_eq!(volume.total_bytes, 1_000);
        assert_eq!(volume.free_bytes, 400);
        assert_eq!(volume.cluster_size, Some(4_096));
        assert_eq!(volume.drive_type, DriveType::Removable);
        assert_eq!(volume.status, VolumeStatus::Available);
        assert!(volume.label.is_empty());
    }

    #[test]
    fn filters_pseudo_file_systems_but_keeps_tmpfs() {
        assert!(is_pseudo("proc"));
        assert!(is_pseudo("SYSFS"));
        assert!(is_pseudo("squashfs"));
        assert!(!is_pseudo("tmpfs"));
        assert!(!is_pseudo("ext4"));
    }

    #[test]
    fn classifies_remote_and_removable_mounts() {
        assert_eq!(
            drive_type(&entry("/mnt/nfs", "nfs4", "server:/export")),
            DriveType::Remote
        );
        assert_eq!(
            drive_type(&entry("/mnt/share", "cifs", "//server/share")),
            DriveType::Remote
        );
        assert_eq!(
            drive_type(&entry("/run/media/user/usb", "vfat", "/dev/sdc1")),
            DriveType::Removable
        );
        assert_eq!(
            drive_type(&entry("/", "ext4", "/dev/sda1")),
            DriveType::Fixed
        );
    }

    #[test]
    fn serial_numbers_are_stable_and_device_specific() {
        assert_eq!(stable_serial("/dev/sda1"), stable_serial("/dev/sda1"));
        assert_ne!(stable_serial("/dev/sda1"), stable_serial("/dev/sda2"));
    }

    #[test]
    fn decodes_udev_labels() {
        assert_eq!(decode_udev_escape(r"My\x20Disk"), "My Disk");
        assert_eq!(decode_udev_escape("plain"), "plain");
    }

    #[test]
    fn parser_keeps_mounts_usable_for_describe() {
        let mounts = parse_mounts("/dev/sda1 / ext4 rw 0 0\n/dev/sdb1 /home ext4 rw 0 0\n");
        let entry = mounts
            .iter()
            .find(|entry| entry.mount_point == Path::new("/home"))
            .expect("home mount");

        assert_eq!(drive_type(entry), DriveType::Fixed);
    }
}
