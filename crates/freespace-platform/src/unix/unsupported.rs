//! Windows-only services with honest `Unsupported` answers on Linux.
//!
//! There is no Restart Manager, NTFS ACL/ADS model, Explorer context menu or
//! Windows shredder on Linux. Returning [`Error::Unsupported`] (which the CLI
//! maps to `E_UNSUPPORTED`) is the contract; emulating these services would
//! promise guarantees the platform cannot keep.

use std::path::Path;

use freespace_core::{Error, Result};

use crate::{
    ContextMenuService, ShredService, UnlockService,
    common::{
        shred::{ShredOptions, ShredOutcome, ShredPreview},
        unlock::FileLock,
    },
};

fn unsupported<T>(capability: &str) -> Result<T> {
    Err(Error::Unsupported(format!(
        "{capability} is not available on Linux"
    )))
}

/// Lock inspection and release through the Windows Restart Manager.
pub(crate) struct UnlockBackend;

impl UnlockService for UnlockBackend {
    fn find_locks(&self, _path: &Path) -> Result<Vec<FileLock>> {
        unsupported("lock inspection")
    }

    fn release_soft(&self, _path: &Path) -> Result<u32> {
        unsupported("lock release")
    }

    fn terminate(&self, _process_id: u32) -> Result<()> {
        unsupported("process termination through the Restart Manager")
    }

    fn delete_on_reboot(&self, _path: &Path) -> Result<()> {
        unsupported("delete-on-reboot")
    }
}

/// Irreversible overwriting, available only in the Windows CLI.
pub(crate) struct ShredBackend;

impl ShredService for ShredBackend {
    fn preview(&self, _path: &Path) -> Result<ShredPreview> {
        unsupported("the shredder")
    }

    fn run(&self, _path: &Path, _options: ShredOptions) -> Result<ShredOutcome> {
        unsupported("the shredder")
    }
}

/// Explorer context-menu integration, which does not exist on Linux desktops.
pub(crate) struct ContextMenuBackend;

impl ContextMenuService for ContextMenuBackend {
    fn set_enabled(&self, _enabled: bool) -> Result<()> {
        unsupported("Explorer context-menu integration")
    }

    fn status(&self) -> Result<bool> {
        Ok(false)
    }

    fn uninstall(&self) -> Result<()> {
        Ok(())
    }
}
