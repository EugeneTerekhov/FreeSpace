//! Windows backend: WinAPI calls and the [`crate::Platform`] composition.

pub mod ads;
pub mod allocated;
pub mod command_line;
#[cfg(feature = "context-menu")]
pub mod context_menu;
pub mod elevation;
pub mod etw;
pub mod file_lock;
pub mod locks;
pub mod mft;
pub mod paths;
pub mod process_io;
pub mod recycle;
pub mod reparse;
pub mod reveal;
pub mod security;
pub mod shred;
pub mod trim;
pub mod volumes;
pub mod win;

use std::{
    ffi::OsString,
    ops::Range,
    path::{Path, PathBuf},
    time::Duration,
};

use freespace_core::Result;

use crate::{
    Capabilities, ContextMenuService, Elevation, ElevationOutcome, InstanceLock, InstanceLockGuard,
    Metadata, MonitorBackend, PathPolicy, Platform, Reveal, ShredService, StorageHints, Trash,
    UnlockService, Volumes,
    common::{
        metadata::{LinkKind, StreamInfo},
        monitor::ProcessIo,
        shred::{ShredOptions, ShredOutcome, ShredPreview},
        storage::StorageKind,
        unlock::FileLock,
        volumes::VolumeInfo,
    },
};

/// The Windows backend selected by [`crate::platform`].
pub static PLATFORM: Platform = Platform {
    paths: &Paths,
    volumes: &VolumesBackend,
    trash: &TrashBackend,
    reveal: &RevealBackend,
    metadata: &MetadataBackend,
    monitor: &MonitorBackendImpl,
    elevation: &ElevationBackend,
    unlock: &UnlockBackend,
    shred: &ShredBackend,
    context_menu: &ContextMenuBackend,
    storage: &StorageBackend,
    instance_lock: &InstanceLockBackend,
    capabilities: Capabilities::windows(),
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
        // Windows file names are case-insensitive.
        paths::display(path).to_lowercase()
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
        volumes::list_volumes()
    }

    fn describe(&self, path: &Path) -> Result<VolumeInfo> {
        volumes::describe_volume(path)
    }
}

struct TrashBackend;

impl Trash for TrashBackend {
    fn trash(&self, paths: &[PathBuf]) -> Result<u32> {
        recycle::recycle(paths)
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
        security::get_attributes(path)
    }

    fn set_attributes(&self, path: &Path, set: u32, clear: u32) -> Result<u32> {
        security::set_attributes(path, set, clear)
    }

    fn owner(&self, path: &Path) -> Result<Option<String>> {
        security::owner(path)
    }

    fn allocated_size(&self, path: &Path) -> Result<u64> {
        allocated::allocated_size(path)
    }

    fn allocated_ranges(&self, path: &Path) -> Result<Vec<Range<u64>>> {
        allocated::allocated_ranges(path)
    }

    fn streams(&self, path: &Path) -> Result<Vec<StreamInfo>> {
        ads::list_streams(path)
    }

    fn link_kind(&self, path: &Path) -> Result<Option<LinkKind>> {
        Ok(reparse::inspect(path)?.map(|info| match info.kind {
            reparse::ReparseKind::Symlink => LinkKind::Symlink,
            reparse::ReparseKind::Junction => LinkKind::Junction,
            reparse::ReparseKind::Other => LinkKind::Other,
        }))
    }
}

struct MonitorBackendImpl;

impl MonitorBackend for MonitorBackendImpl {
    fn snapshot(&self) -> Result<Vec<ProcessIo>> {
        process_io::snapshot()
    }

    fn counters_for(&self, pid: u32) -> Result<ProcessIo> {
        process_io::counters_for(pid)
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
        Ok(match elevation::relaunch(executable, arguments)? {
            elevation::ElevationResult::Launched => ElevationOutcome::Launched,
            elevation::ElevationResult::Declined => ElevationOutcome::Declined,
        })
    }

    fn relaunch_arguments(&self) -> Vec<OsString> {
        elevation::relaunch_arguments()
    }

    fn current_executable(&self) -> Option<PathBuf> {
        elevation::current_executable()
    }
}

struct UnlockBackend;

impl UnlockService for UnlockBackend {
    fn find_locks(&self, path: &Path) -> Result<Vec<FileLock>> {
        locks::find_locks(path)
    }

    fn release_soft(&self, path: &Path) -> Result<u32> {
        locks::release_soft(path)
    }

    fn terminate(&self, process_id: u32) -> Result<()> {
        locks::terminate_process(process_id)
    }

    fn delete_on_reboot(&self, path: &Path) -> Result<()> {
        locks::schedule_delete_on_reboot(path)
    }
}

struct ShredBackend;

impl ShredService for ShredBackend {
    fn preview(&self, path: &Path) -> Result<ShredPreview> {
        shred::preview(path)
    }

    fn run(&self, path: &Path, options: ShredOptions) -> Result<ShredOutcome> {
        shred::run(path, options)
    }
}

struct ContextMenuBackend;

impl ContextMenuService for ContextMenuBackend {
    fn set_enabled(&self, enabled: bool) -> Result<()> {
        #[cfg(feature = "context-menu")]
        {
            context_menu::ContextMenu::set_enabled(enabled)?;
            Ok(())
        }

        #[cfg(not(feature = "context-menu"))]
        {
            let _ = enabled;
            Err(freespace_core::Error::Unsupported(
                "the context-menu feature is not compiled in".to_owned(),
            ))
        }
    }

    fn status(&self) -> Result<bool> {
        #[cfg(feature = "context-menu")]
        {
            context_menu::ContextMenu::user().is_installed()
        }

        #[cfg(not(feature = "context-menu"))]
        {
            Ok(false)
        }
    }

    fn uninstall(&self) -> Result<()> {
        #[cfg(feature = "context-menu")]
        {
            context_menu::ContextMenu::user().uninstall()
        }

        #[cfg(not(feature = "context-menu"))]
        {
            Ok(())
        }
    }
}

struct StorageBackend;

impl StorageHints for StorageBackend {
    fn kind(&self, path: &Path) -> Result<StorageKind> {
        trim::storage_kind(path)
    }

    fn trim(&self, path: &Path) -> Result<()> {
        trim::trim_file(path)
    }
}

struct InstanceLockBackend;

impl InstanceLock for InstanceLockBackend {
    fn acquire(&self, path: &Path, timeout: Duration) -> Result<Box<dyn InstanceLockGuard>> {
        Ok(Box::new(WindowsInstanceLock(file_lock::FileLock::acquire(
            path, timeout,
        )?)))
    }
}

struct WindowsInstanceLock(file_lock::FileLock);

impl InstanceLockGuard for WindowsInstanceLock {
    fn path(&self) -> &Path {
        self.0.path()
    }

    fn write_metadata(&mut self, contents: &str) -> Result<()> {
        self.0.write_metadata(contents)
    }
}
