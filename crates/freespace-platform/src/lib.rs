//! Platform layer for FreeSpace.
//!
//! Every operating-system specific call lives in this crate. The rest of the
//! workspace talks to [`platform()`], which returns a [`Platform`] of narrow
//! object-safe traits, and reads [`Capabilities`] to hide unsupported actions.
//!
//! The composition root below is the only place where `cfg(windows)` and
//! `cfg(unix)` choose a backend; business logic never branches on the operating
//! system.
//!
//! The original facade modules (`paths`, `volumes`, `security`, `recycle`) are
//! kept as thin delegates so existing call sites keep working while the code
//! migrates to [`platform()`] directly.

use std::{
    ops::Range,
    path::{Path, PathBuf},
    time::Duration,
};

use freespace_core::Result;

use crate::common::{
    metadata::{LinkKind, StreamInfo},
    monitor::ProcessIo,
    shred::{ShredOptions, ShredOutcome, ShredPreview},
    storage::StorageKind,
    unlock::FileLock,
    volumes::VolumeInfo,
};

pub mod capabilities;
pub mod common;
pub mod paths;
pub mod recycle;
pub mod security;
pub mod volumes;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

pub use capabilities::{Capabilities, PlatformId, ScanMethods};

#[cfg(windows)]
pub use windows::{
    ads, allocated, command_line, elevation, etw, file_lock, locks, mft, process_io, reparse,
    reveal, trim, win,
};

#[cfg(unix)]
pub use unix::guarded;

#[cfg(all(windows, feature = "context-menu"))]
pub use windows::context_menu;

/// Path normalisation, validation and scope checks.
pub trait PathPolicy: Send + Sync {
    /// Resolves a path to the platform's canonical absolute form.
    fn normalize(&self, path: &Path) -> Result<PathBuf>;
    /// Renders a path for logs and reports.
    fn display(&self, path: &Path) -> String;
    /// Rejects a path that exceeds the platform limit before any change.
    fn validate_length(&self, path: &Path) -> Result<()>;
    /// Comparison key that follows the platform's case rules.
    fn equivalent_key(&self, path: &Path) -> String;
    /// Returns the root of the volume that contains `path`.
    fn volume_root(&self, path: &Path) -> Result<PathBuf>;
    /// Joins a file name and validates the resulting length.
    fn join_within_limit(&self, parent: &Path, name: &str) -> Result<PathBuf>;
    /// Verifies that `target` lies inside `root` (or is the root itself).
    fn ensure_within(&self, root: &Path, target: &Path) -> Result<()>;
}

/// Logical-volume enumeration.
pub trait Volumes: Send + Sync {
    /// Lists every mounted volume.
    fn list(&self) -> Result<Vec<VolumeInfo>>;
    /// Describes the volume that contains `path`.
    fn describe(&self, path: &Path) -> Result<VolumeInfo>;
}

/// System trash (Recycle Bin or XDG trash).
pub trait Trash: Send + Sync {
    /// Moves `paths` to the system trash and returns how many disappeared.
    fn trash(&self, paths: &[PathBuf]) -> Result<u32>;
}

/// Opening an object in the platform's file manager.
pub trait Reveal: Send + Sync {
    /// Shows `path` in the file manager without changing it.
    fn reveal(&self, path: &Path) -> Result<()>;
}

/// File metadata: attributes, owner, allocation, streams and links.
pub trait Metadata: Send + Sync {
    /// Reads the platform attribute mask.
    fn attributes(&self, path: &Path) -> Result<u32>;
    /// Applies `set` bits and clears `clear` bits.
    fn set_attributes(&self, path: &Path, set: u32, clear: u32) -> Result<u32>;
    /// Returns the owner name, or a platform-native identifier.
    fn owner(&self, path: &Path) -> Result<Option<String>>;
    /// Returns the space actually allocated to `path`.
    fn allocated_size(&self, path: &Path) -> Result<u64>;
    /// Returns the byte ranges that are actually allocated.
    fn allocated_ranges(&self, path: &Path) -> Result<Vec<Range<u64>>>;
    /// Lists alternate data streams (empty where the concept does not exist).
    fn streams(&self, path: &Path) -> Result<Vec<StreamInfo>>;
    /// Classifies a link, returning `None` for a regular object.
    fn link_kind(&self, path: &Path) -> Result<Option<LinkKind>>;
}

/// Per-process I/O counters used by the write monitor.
pub trait MonitorBackend: Send + Sync {
    /// Snapshots the counters of every readable process.
    fn snapshot(&self) -> Result<Vec<ProcessIo>>;
    /// Reads the counters of one process.
    fn counters_for(&self, pid: u32) -> Result<ProcessIo>;
}

/// Elevation state and on-demand relaunch.
pub trait Elevation: Send + Sync {
    /// Returns `true` when the current process is elevated.
    fn is_elevated(&self) -> bool;
    /// Returns `true` when the current process belongs to the administrators.
    fn is_administrator(&self) -> bool;
    /// Relaunches the executable with elevated rights.
    fn relaunch(
        &self,
        executable: &Path,
        arguments: &[std::ffi::OsString],
    ) -> Result<ElevationOutcome>;
    /// Arguments that restart the current binary with the same command line.
    fn relaunch_arguments(&self) -> Vec<std::ffi::OsString>;
    /// Path of the running executable, when it can be resolved.
    fn current_executable(&self) -> Option<PathBuf>;
}

/// Outcome of an elevation request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElevationOutcome {
    /// A second, elevated instance was started.
    Launched,
    /// The user dismissed the elevation prompt.
    Declined,
}

impl ElevationOutcome {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Launched => "launched",
            Self::Declined => "declined",
        }
    }
}

/// Lock inspection and release.
pub trait UnlockService: Send + Sync {
    /// Lists the processes that hold `path`.
    fn find_locks(&self, path: &Path) -> Result<Vec<FileLock>>;
    /// Asks the holders to close the file and returns how many were asked.
    fn release_soft(&self, path: &Path) -> Result<u32>;
    /// Terminates one process.
    fn terminate(&self, process_id: u32) -> Result<()>;
    /// Schedules deletion of `path` after the next reboot.
    fn delete_on_reboot(&self, path: &Path) -> Result<()>;
}

/// Irreversible file destruction.
pub trait ShredService: Send + Sync {
    /// Describes what shredding `path` would do.
    fn preview(&self, path: &Path) -> Result<ShredPreview>;
    /// Overwrites and removes `path`.
    fn run(&self, path: &Path, options: ShredOptions) -> Result<ShredOutcome>;
}

/// Explorer context-menu integration.
pub trait ContextMenuService: Send + Sync {
    /// Enables or disables the current-user integration.
    fn set_enabled(&self, enabled: bool) -> Result<()>;
    /// Returns `true` when the integration is installed.
    fn status(&self) -> Result<bool>;
    /// Removes every registered entry.
    fn uninstall(&self) -> Result<()>;
}

/// Storage media hints and TRIM.
pub trait StorageHints: Send + Sync {
    /// Detects the media kind of the volume that contains `path`.
    fn kind(&self, path: &Path) -> Result<StorageKind>;
    /// Asks the file system to discard the clusters of a file.
    fn trim(&self, path: &Path) -> Result<()>;
}

/// Cross-process instance lock.
pub trait InstanceLock: Send + Sync {
    /// Acquires the lock, waiting up to `timeout`.
    fn acquire(&self, path: &Path, timeout: Duration) -> Result<Box<dyn InstanceLockGuard>>;
}

/// Held instance lock; released on drop.
pub trait InstanceLockGuard: Send + Sync {
    /// Path of the lock file.
    fn path(&self) -> &Path;
    /// Writes a small diagnostic record into the lock file.
    fn write_metadata(&mut self, contents: &str) -> Result<()>;
}

/// Every platform capability behind one value.
pub struct Platform {
    /// Path rules.
    pub paths: &'static dyn PathPolicy,
    /// Volume enumeration.
    pub volumes: &'static dyn Volumes,
    /// System trash.
    pub trash: &'static dyn Trash,
    /// File-manager reveal.
    pub reveal: &'static dyn Reveal,
    /// File metadata.
    pub metadata: &'static dyn Metadata,
    /// Write-monitor backend.
    pub monitor: &'static dyn MonitorBackend,
    /// Elevation.
    pub elevation: &'static dyn Elevation,
    /// Lock inspection and release.
    pub unlock: &'static dyn UnlockService,
    /// Shredder.
    pub shred: &'static dyn ShredService,
    /// Context-menu integration.
    pub context_menu: &'static dyn ContextMenuService,
    /// Storage hints.
    pub storage: &'static dyn StorageHints,
    /// Instance lock.
    pub instance_lock: &'static dyn InstanceLock,
    /// Advertised capabilities.
    pub capabilities: Capabilities,
}

impl std::fmt::Debug for Platform {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Platform")
            .field("capabilities", &self.capabilities)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod contract_tests;

/// Returns the backend selected for the current build target.
#[cfg(windows)]
#[must_use]
pub fn platform() -> &'static Platform {
    &windows::PLATFORM
}

/// Returns the backend selected for the current build target.
#[cfg(unix)]
#[must_use]
pub fn platform() -> &'static Platform {
    &unix::PLATFORM
}
