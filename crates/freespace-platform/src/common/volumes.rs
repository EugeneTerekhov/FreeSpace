//! Platform-neutral volume records.

use std::path::PathBuf;

use freespace_core::model::{FileSystem, Volume, VolumeStatus};

/// Type of a logical drive.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DriveType {
    /// Type could not be determined.
    #[default]
    Unknown,
    /// The root path is invalid (for example, no volume is mounted).
    NoRootDirectory,
    /// Removable media.
    Removable,
    /// Fixed media.
    Fixed,
    /// Network drive.
    Remote,
    /// CD-ROM, DVD-ROM or Blu-ray drive.
    CdRom,
    /// RAM disk.
    RamDisk,
}

impl DriveType {
    /// Maps the raw `GetDriveTypeW` value.
    #[must_use]
    pub const fn from_raw(value: u32) -> Self {
        match value {
            1 => Self::NoRootDirectory,
            2 => Self::Removable,
            3 => Self::Fixed,
            4 => Self::Remote,
            5 => Self::CdRom,
            6 => Self::RamDisk,
            _ => Self::Unknown,
        }
    }

    /// Returns `true` for removable media.
    #[must_use]
    pub const fn is_removable(self) -> bool {
        matches!(self, Self::Removable)
    }

    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::NoRootDirectory => "no-root-directory",
            Self::Removable => "removable",
            Self::Fixed => "fixed",
            Self::Remote => "remote",
            Self::CdRom => "cdrom",
            Self::RamDisk => "ramdisk",
        }
    }
}

/// Information about one logical drive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeInfo {
    /// Drive root (`C:\` or `/`).
    pub mount_point: PathBuf,
    /// Volume label; empty when the volume has none.
    pub label: String,
    /// Reported file system.
    pub file_system: FileSystem,
    /// Total volume size in bytes.
    pub total_bytes: u64,
    /// Space available to the current user.
    pub free_bytes: u64,
    /// Cluster size in bytes, when it could be queried.
    pub cluster_size: Option<u32>,
    /// Volume serial number, used as part of hash-cache keys.
    pub serial_number: u32,
    /// Drive type.
    pub drive_type: DriveType,
    /// Availability.
    pub status: VolumeStatus,
}

impl VolumeInfo {
    /// Converts the platform record into the domain model.
    #[must_use]
    pub fn into_volume(self) -> Volume {
        Volume {
            id: Volume::id_for_mount_point(&self.mount_point),
            mount_point: self.mount_point,
            label: self.label,
            file_system: self.file_system,
            total_bytes: self.total_bytes,
            free_bytes: self.free_bytes,
            cluster_size: self.cluster_size,
            scan_method: None,
            is_elevated: false,
            is_removable: self.drive_type.is_removable(),
            status: self.status,
        }
    }
}
