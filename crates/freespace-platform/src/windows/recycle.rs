//! Recycle Bin deletion through `IFileOperation` (FR-043).
//!
//! `SHFileOperation` is a legacy API; the modern shell file operation supports
//! long paths, undo records and returns a reliable "was it aborted" flag. The
//! user always gets the Recycle Bin recovery path unless they deliberately ask
//! for quarantine or permanent deletion.

use std::path::{Path, PathBuf};

use freespace_core::{Error, Result};
use windows::{
    Win32::{
        System::Com::{
            CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
        },
        UI::Shell::{
            FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, FileOperation,
            IFileOperation, IShellItem, SHCreateItemFromParsingName,
        },
    },
    core::{HRESULT, PCWSTR},
};

use super::{paths, win};

/// `RPC_E_CHANGED_MODE`: COM is already initialised with another apartment
/// model, which is fine for this call.
const RPC_E_CHANGED_MODE: i32 = 0x8001_0106_u32 as i32;

/// Deletes `paths` to the Recycle Bin.
///
/// Returns the number of objects that disappeared from their original paths.
///
/// # Errors
/// Returns [`Error::InvalidArgument`] for an empty list, [`Error::Cancelled`]
/// when the shell reports an aborted operation, [`Error::NotFound`] when an
/// object disappears before the operation starts, and [`Error::Io`] for COM or
/// shell failures.
pub fn recycle(paths: &[PathBuf]) -> Result<u32> {
    if paths.is_empty() {
        return Err(Error::InvalidArgument(
            "the Recycle Bin needs at least one object".to_owned(),
        ));
    }

    let _com = ComGuard::new()?;

    // SAFETY: `FileOperation` is a registered shell class and `IFileOperation`
    // is the interface it implements.
    let operation: IFileOperation =
        unsafe { CoCreateInstance(&FileOperation, None::<&windows::core::IUnknown>, CLSCTX_ALL) }
            .map_err(|error| Error::Io(win::win_error(&error)))?;

    // SAFETY: `operation` is a valid COM interface pointer for this scope.
    unsafe {
        operation.SetOperationFlags(FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI)
    }
    .map_err(|error| Error::Io(win::win_error(&error)))?;

    for path in paths {
        let verbatim = paths::to_verbatim(path)?;
        // The shell namespace does not understand the `\\?\` prefix; it is
        // long-path aware on Windows 10 and later, so the display form is safe.
        let display = paths::strip_verbatim(&verbatim);
        let wide = win::to_wide(display.as_os_str());
        // SAFETY: `wide` is NUL-terminated and lives for the call.
        let item: IShellItem = unsafe {
            SHCreateItemFromParsingName(
                PCWSTR(wide.as_ptr()),
                None::<&windows::Win32::System::Com::IBindCtx>,
            )
        }
        .map_err(|error| Error::Io(win::win_error(&error)))?;
        // SAFETY: the item is a valid shell item owned by this scope.
        unsafe {
            operation.DeleteItem(
                &item,
                None::<&windows::Win32::UI::Shell::IFileOperationProgressSink>,
            )
        }
        .map_err(|error| Error::Io(win::win_error(&error)))?;
    }

    // SAFETY: the queued items are owned by the operation.
    let performed = unsafe { operation.PerformOperations() };

    let mut removed = 0u32;
    for path in paths {
        if !exists(path) {
            removed = removed.saturating_add(1);
        }
    }

    // The file-system state is the truth. The shell can return an error or an
    // aborted flag after it has actually moved the object; reporting a failure
    // then would leave the row in the tree even though the data is gone.
    let total = u32::try_from(paths.len()).unwrap_or(u32::MAX);
    if removed == total {
        return Ok(removed);
    }

    performed.map_err(|error| Error::Io(win::win_error(&error)))?;

    // SAFETY: `operation` is still alive and fully initialised.
    let aborted = unsafe { operation.GetAnyOperationsAborted() }
        .map_err(|error| Error::Io(win::win_error(&error)))?;
    if aborted.as_bool() {
        return Err(Error::Cancelled(
            "the shell aborted the Recycle Bin operation".to_owned(),
        ));
    }

    // The shell reported success but did not move every object (for example
    // when the Recycle Bin refuses it). Never turn that into a successful
    // deletion: the caller would remove the row while the data stays.
    Err(Error::Operation(format!(
        "{} of {total} objects were not moved to the Recycle Bin",
        total.saturating_sub(removed)
    )))
}

fn exists(path: &Path) -> bool {
    let verbatim = paths::to_verbatim(path).unwrap_or_else(|_| path.to_path_buf());

    std::fs::symlink_metadata(verbatim).is_ok()
}

/// COM apartment guard. COM is initialised lazily and only de-initialised when
/// this call is the one that initialised it.
struct ComGuard {
    uninitialize: bool,
}

impl ComGuard {
    fn new() -> Result<Self> {
        // SAFETY: `CoInitializeEx` has no preconditions. A changed-mode HRESULT
        // means another component already owns the apartment.
        let result: HRESULT = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };

        if result.is_ok() {
            return Ok(Self { uninitialize: true });
        }
        if result.0 == RPC_E_CHANGED_MODE {
            return Ok(Self {
                uninitialize: false,
            });
        }

        Err(Error::Io(std::io::Error::from_raw_os_error(result.0)))
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.uninitialize {
            // SAFETY: this guard initialised the apartment exactly once.
            unsafe { CoUninitialize() };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::recycle;
    use freespace_core::ErrorCode;

    #[test]
    fn rejects_empty_input() {
        let error = recycle(&[]).expect_err("must reject");

        assert_eq!(error.code(), ErrorCode::InvalidArgument);
    }

    #[test]
    fn moves_a_fixture_to_the_recycle_bin() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("disposable.txt");
        std::fs::write(&path, b"payload").expect("write fixture");

        match recycle(std::slice::from_ref(&path)) {
            Ok(count) => {
                assert_eq!(count, 1);
                assert!(!path.exists());
            }
            Err(error) if error.code() == ErrorCode::Cancelled => {
                assert!(path.exists(), "an aborted operation keeps the file");
            }
            Err(error) => panic!("unexpected Recycle Bin error: {error}"),
        }
    }
}
