//! Opening an object in the desktop file manager through `xdg-open`.
//!
//! Like the Windows backend this is a presentation concern: it starts the
//! user's handler and returns; FreeSpace never reads or changes the object. A
//! missing object is rejected before a process is spawned so the command cannot
//! silently open an unrelated parent directory.

use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
};

use freespace_core::{Error, Result};

use super::paths;

/// Opens `path` with the default desktop handler.
///
/// # Errors
/// Returns [`Error::NotFound`] when the object no longer exists,
/// [`Error::AccessDenied`] when its metadata cannot be read and [`Error::Io`]
/// when `xdg-open` cannot be started.
pub fn reveal(path: &Path) -> Result<()> {
    fs::metadata(path).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => Error::NotFound(paths::display(path)),
        std::io::ErrorKind::PermissionDenied => Error::AccessDenied(paths::display(path)),
        _ => Error::Io(error),
    })?;

    Command::new("xdg-open")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(Error::Io)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::reveal;

    #[test]
    fn reports_a_missing_object_without_starting_a_handler() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("gone.bin");

        let error = reveal(&path).expect_err("missing object");

        assert_eq!(error.code(), freespace_core::ErrorCode::NotFound);
    }
}
