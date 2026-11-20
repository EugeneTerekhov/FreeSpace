//! File locks and unlocking — portable stub.
//!
//! Lock discovery and release use the Windows Restart Manager and process
//! termination, so the implementation lives in `unlock.rs` and is compiled on
//! Windows only. The same public API stays available on other platforms and
//! returns `E_UNSUPPORTED` (the `unlock` capability is `false`).

use std::path::Path;

use freespace_ops::Store;

use crate::{
    dto::{FileLock, UnlockMode, UnlockResult},
    error::{UiError, UiErrorCode},
};

/// Lists the applications that currently hold `path`.
///
/// # Errors
/// Always returns `E_UNSUPPORTED` on this platform.
pub fn locks(path: &Path) -> Result<Vec<FileLock>, UiError> {
    let _ = path;
    unsupported()
}

/// Releases the locks on `path`.
///
/// # Errors
/// Always returns `E_UNSUPPORTED` on this platform.
#[allow(clippy::too_many_arguments)]
pub fn release(
    path: &Path,
    mode: UnlockMode,
    process_ids: &[u32],
    confirmed: bool,
    elevated: bool,
    store: Option<&Store>,
) -> Result<UnlockResult, UiError> {
    let _ = (path, mode, process_ids, confirmed, elevated, store);
    unsupported()
}

/// Registers `path` in the reboot deletion queue.
///
/// # Errors
/// Always returns `E_UNSUPPORTED` on this platform.
pub fn schedule_reboot(
    path: &Path,
    confirmed: bool,
    elevated: bool,
    store: Option<&Store>,
) -> Result<UnlockResult, UiError> {
    let _ = (path, confirmed, elevated, store);
    unsupported()
}

fn unsupported<T>() -> Result<T, UiError> {
    Err(UiError::new(UiErrorCode::Unsupported)
        .with_details("lock inspection and release are available on Windows only"))
}

#[cfg(test)]
mod tests {
    use super::{locks, release, schedule_reboot};
    use crate::dto::UnlockMode;
    use crate::error::UiErrorCode;

    #[test]
    fn every_unlock_operation_is_unsupported() {
        assert_eq!(
            locks(std::path::Path::new("/tmp")).expect_err("locks").code,
            UiErrorCode::Unsupported
        );
        assert_eq!(
            release(
                std::path::Path::new("/tmp"),
                UnlockMode::Soft,
                &[],
                true,
                false,
                None,
            )
            .expect_err("release")
            .code,
            UiErrorCode::Unsupported
        );
        assert_eq!(
            schedule_reboot(std::path::Path::new("/tmp"), true, true, None)
                .expect_err("reboot")
                .code,
            UiErrorCode::Unsupported
        );
    }
}
