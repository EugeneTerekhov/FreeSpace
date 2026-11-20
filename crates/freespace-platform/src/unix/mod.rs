//! Unix backend: POSIX, `/proc`, sysfs and XDG trash behind the common traits.
//!
//! Windows-only capabilities answer [`freespace_core::Error::Unsupported`] in
//! [`unsupported`] and in the metadata module; everything the platform can
//! really do is implemented here. The backend is selected once in
//! [`crate::platform`]; business logic never names a Unix module directly.
//!
//! * [`paths`] — `PATH_MAX`, case-sensitive keys, mount-aware volume roots.
//! * [`volumes`] — `/proc/self/mounts` plus `statvfs`.
//! * [`trash`] — freedesktop XDG trash with `.trashinfo` records.
//! * [`metadata`] — `st_blocks`, owners from `/etc/passwd`, symbolic links.
//! * [`monitor`] — per-process counters from `/proc/<pid>/io`.
//! * [`elevation`], [`reveal`], [`instance_lock`] — session services.
//! * [`storage`] — media kind from sysfs.
//! * [`guarded`] — protected system paths.

pub mod guarded;

mod elevation;
mod instance_lock;
mod metadata;
mod monitor;
mod paths;
mod reveal;
mod storage;
mod trash;
mod unsupported;
mod volumes;

use std::{
    ffi::OsString,
    ops::Range,
    path::{Path, PathBuf},
    time::Duration,
};

use freespace_core::Result;

use crate::{
    Capabilities, Elevation, ElevationOutcome, InstanceLock, InstanceLockGuard, Metadata,
    MonitorBackend, PathPolicy, Platform, Reveal, StorageHints, Trash, Volumes,
    common::{
        metadata::{LinkKind, StreamInfo},
        monitor::ProcessIo,
        storage::StorageKind,
        volumes::VolumeInfo,
    },
};

/// The Unix backend selected by [`crate::platform`].
pub static PLATFORM: Platform = Platform {
    paths: &Paths,
    volumes: &VolumesBackend,
    trash: &TrashBackend,
    reveal: &RevealBackend,
    metadata: &MetadataBackend,
    monitor: &MonitorBackendImpl,
    elevation: &ElevationBackend,
    unlock: &unsupported::UnlockBackend,
    shred: &unsupported::ShredBackend,
    context_menu: &unsupported::ContextMenuBackend,
    storage: &StorageBackend,
    instance_lock: &InstanceLockBackend,
    capabilities: Capabilities::linux(),
};

struct Paths;

impl PathPolicy for Paths {
    fn normalize(&self, path: &Path) -> Result<PathBuf> {
        paths::normalize(path)
    }

    fn display(&self, path: &Path) -> String {
        paths::display(path)
    }

    fn validate_length(&self, path: &Path) -> Result<()> {
        paths::validate_length(path)
    }

    fn equivalent_key(&self, path: &Path) -> String {
        paths::equivalent_key(path)
    }

    fn volume_root(&self, path: &Path) -> Result<PathBuf> {
        paths::volume_root(path)
    }

    fn join_within_limit(&self, parent: &Path, name: &str) -> Result<PathBuf> {
        paths::join_within_limit(parent, name)
    }

    fn ensure_within(&self, root: &Path, target: &Path) -> Result<()> {
        paths::ensure_within(root, target)
    }
}

struct VolumesBackend;

impl Volumes for VolumesBackend {
    fn list(&self) -> Result<Vec<VolumeInfo>> {
        volumes::list()
    }

    fn describe(&self, path: &Path) -> Result<VolumeInfo> {
        volumes::describe(path)
    }
}

struct TrashBackend;

impl Trash for TrashBackend {
    fn trash(&self, paths: &[PathBuf]) -> Result<u32> {
        trash::trash(paths)
    }
}

struct RevealBackend;

impl Reveal for RevealBackend {
    fn reveal(&self, path: &Path) -> Result<()> {
        reveal::reveal(path)
    }
}

struct MetadataBackend;

impl Metadata for MetadataBackend {
    fn attributes(&self, path: &Path) -> Result<u32> {
        metadata::attributes(path)
    }

    fn set_attributes(&self, path: &Path, set: u32, clear: u32) -> Result<u32> {
        metadata::set_attributes(path, set, clear)
    }

    fn owner(&self, path: &Path) -> Result<Option<String>> {
        metadata::owner(path)
    }

    fn allocated_size(&self, path: &Path) -> Result<u64> {
        metadata::allocated_size(path)
    }

    fn allocated_ranges(&self, path: &Path) -> Result<Vec<Range<u64>>> {
        metadata::allocated_ranges(path)
    }

    fn streams(&self, path: &Path) -> Result<Vec<StreamInfo>> {
        metadata::streams(path)
    }

    fn link_kind(&self, path: &Path) -> Result<Option<LinkKind>> {
        metadata::link_kind(path)
    }
}

struct MonitorBackendImpl;

impl MonitorBackend for MonitorBackendImpl {
    fn snapshot(&self) -> Result<Vec<ProcessIo>> {
        monitor::snapshot()
    }

    fn counters_for(&self, pid: u32) -> Result<ProcessIo> {
        monitor::counters_for(pid)
    }
}

struct ElevationBackend;

impl Elevation for ElevationBackend {
    fn is_elevated(&self) -> bool {
        elevation::is_elevated()
    }

    fn is_administrator(&self) -> bool {
        elevation::is_administrator()
    }

    fn relaunch(&self, executable: &Path, arguments: &[OsString]) -> Result<ElevationOutcome> {
        elevation::relaunch(executable, arguments)
    }

    fn relaunch_arguments(&self) -> Vec<OsString> {
        elevation::relaunch_arguments()
    }

    fn current_executable(&self) -> Option<PathBuf> {
        elevation::current_executable()
    }
}

struct StorageBackend;

impl StorageHints for StorageBackend {
    fn kind(&self, path: &Path) -> Result<StorageKind> {
        storage::kind(path)
    }

    fn trim(&self, path: &Path) -> Result<()> {
        storage::trim(path)
    }
}

struct InstanceLockBackend;

impl InstanceLock for InstanceLockBackend {
    fn acquire(&self, path: &Path, timeout: Duration) -> Result<Box<dyn InstanceLockGuard>> {
        instance_lock::acquire(path, timeout)
    }
}

#[cfg(test)]
mod contract_tests;
