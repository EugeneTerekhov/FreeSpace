//! Opening an object in Windows Explorer.
//!
//! The tree context menu offers «Открыть в Проводнике». A directory is opened
//! as a folder; a file is shown selected inside its parent. The shell is reached
//! through `explorer.exe`, so this stays a presentation concern: no file is read
//! or changed and no shell extension is required.
//!
//! `explorer.exe` does not understand the `\\?\` prefix, therefore the path is
//! converted back to its display form right before the call, exactly like the
//! Recycle Bin wrapper does.

use std::{
    ffi::OsString,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::Command,
};

use freespace_core::{Error, Result};

use super::paths;

/// Opens `path` in Explorer.
///
/// A directory opens as a folder; a file opens its parent with the file
/// selected.
///
/// # Errors
/// Returns [`Error::NotFound`] when the object no longer exists,
/// [`Error::AccessDenied`] when its metadata cannot be read and [`Error::Io`]
/// when Explorer cannot be started.
pub fn reveal(path: &Path) -> Result<()> {
    let verbatim = paths::to_verbatim(path)?;
    let metadata = std::fs::metadata(&verbatim).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => Error::NotFound(paths::display(&verbatim)),
        std::io::ErrorKind::PermissionDenied => Error::AccessDenied(paths::display(&verbatim)),
        _ => Error::Io(error),
    })?;
    let display = paths::strip_verbatim(&verbatim);
    let mut command = Command::new(explorer_program());
    if metadata.is_dir() {
        command.arg(&display);
    } else {
        command.raw_arg(select_argument(&display.to_string_lossy()));
    }
    command.spawn().map_err(Error::Io)?;

    Ok(())
}

/// Absolute path of the system Explorer.
///
/// Relying on the bare name would let a stray `explorer.exe` beside the
/// application win the Windows process search order.
fn explorer_program() -> OsString {
    std::env::var_os("SystemRoot")
        .map(|root| PathBuf::from(root).join("explorer.exe").into_os_string())
        .unwrap_or_else(|| OsString::from("explorer.exe"))
}

/// Argument that selects one object inside its parent folder.
///
/// Explorer requires `/select,` and the quoted path to be one argument; the
/// quotes survive commas and spaces in the name.
fn select_argument(display: &str) -> String {
    format!("/select,\"{display}\"")
}

#[cfg(test)]
mod tests {
    use super::{explorer_program, select_argument};

    #[test]
    fn starts_the_system_explorer() {
        if let Some(root) = std::env::var_os("SystemRoot") {
            let program = explorer_program();
            assert!(std::path::Path::new(&program).starts_with(std::path::Path::new(&root)));
        }
    }

    #[test]
    fn selects_the_object_inside_its_parent() {
        assert_eq!(
            select_argument(r"C:\Users\Public\report 2026, final.pdf"),
            r#"/select,"C:\Users\Public\report 2026, final.pdf""#
        );
    }

    #[test]
    fn reports_a_missing_object_without_starting_explorer() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("gone.bin");

        let error = super::reveal(&path).expect_err("missing object");

        assert_eq!(error.code(), freespace_core::ErrorCode::NotFound);
    }
}
