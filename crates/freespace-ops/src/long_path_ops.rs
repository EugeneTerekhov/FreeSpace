//! File operations for paths longer than the legacy `MAX_PATH` (FR-051…FR-053).
//!
//! Every operation converts its paths to verbatim form and validates the target
//! length *before* touching the file system, so an over-long result fails with a
//! clear error instead of leaving a half-renamed object behind.
//!
//! Verbatim paths are a Windows concept; on Linux the whole module reports
//! [`Error::Unsupported`] and callers use the regular file-system APIs instead.

use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::{fs, io};

use freespace_core::{Error, Result};
#[cfg(windows)]
use freespace_platform::paths;

/// Which operation ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LongPathOperation {
    /// Rename in place.
    Rename,
    /// Move to another directory.
    Move,
    /// Copy to another directory.
    Copy,
    /// Delete permanently.
    Delete,
}

impl LongPathOperation {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rename => "rename",
            Self::Move => "move",
            Self::Copy => "copy",
            Self::Delete => "delete",
        }
    }
}

/// Result of a long-path operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LongPathOutcome {
    /// Operation.
    pub operation: LongPathOperation,
    /// Original path.
    pub from: PathBuf,
    /// Destination path, when the operation has one.
    pub to: Option<PathBuf>,
    /// Bytes moved, copied or removed.
    pub bytes: u64,
}

#[cfg(not(windows))]
fn unsupported<T>() -> Result<T> {
    Err(Error::Unsupported(
        "long-path operations are available on Windows only".to_owned(),
    ))
}

/// Renames an object inside its directory.
///
/// # Errors
/// Returns [`Error::NotFound`] for a missing source, [`Error::Conflict`] when
/// the target exists, and [`Error::InvalidPath`] when the resulting name would
/// exceed the supported limit. On non-Windows platforms it returns
/// [`Error::Unsupported`].
pub fn rename(from: &Path, new_name: &str) -> Result<LongPathOutcome> {
    #[cfg(not(windows))]
    {
        let _ = (from, new_name);
        unsupported()
    }

    #[cfg(windows)]
    {
        let source = paths::normalize(from)?;
        let parent = source.parent().ok_or_else(|| Error::InvalidPath {
            path: source.clone(),
            reason: "the object has no parent directory".to_owned(),
        })?;
        let target = paths::join_within_limit(&paths::strip_verbatim(parent), new_name)?;
        ensure_absent(&source, &target)?;
        let bytes = size_of(&source)?;

        fs::rename(paths::to_verbatim(&source)?, paths::to_verbatim(&target)?)?;

        Ok(LongPathOutcome {
            operation: LongPathOperation::Rename,
            from: from.to_path_buf(),
            to: Some(target),
            bytes,
        })
    }
}

/// Moves an object into another directory, keeping its name.
///
/// # Errors
/// Returns an error for a missing source or destination directory, a conflict,
/// or a target beyond the supported path limit. On non-Windows platforms it
/// returns [`Error::Unsupported`].
pub fn move_to(from: &Path, destination_dir: &Path) -> Result<LongPathOutcome> {
    #[cfg(not(windows))]
    {
        let _ = (from, destination_dir);
        unsupported()
    }

    #[cfg(windows)]
    {
        let source = paths::normalize(from)?;
        let destination = paths::normalize(destination_dir)?;
        if !fs::metadata(paths::to_verbatim(&destination)?)?.is_dir() {
            return Err(Error::InvalidArgument(format!(
                "`{}` is not a directory",
                paths::display(&destination)
            )));
        }
        let name = source
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| Error::InvalidPath {
                path: source.clone(),
                reason: "the object has no usable file name".to_owned(),
            })?;
        let target = paths::join_within_limit(&paths::strip_verbatim(&destination), name)?;
        ensure_absent(&source, &target)?;
        let bytes = size_of(&source)?;

        fs::rename(paths::to_verbatim(&source)?, paths::to_verbatim(&target)?)?;

        Ok(LongPathOutcome {
            operation: LongPathOperation::Move,
            from: from.to_path_buf(),
            to: Some(target),
            bytes,
        })
    }
}

/// Copies an object into another directory, keeping its name.
///
/// # Errors
/// Returns an error for a missing source or destination, a conflict, a copy
/// into the object's own subtree, or a target beyond the supported limit. On
/// non-Windows platforms it returns [`Error::Unsupported`].
pub fn copy_to(from: &Path, destination_dir: &Path) -> Result<LongPathOutcome> {
    #[cfg(not(windows))]
    {
        let _ = (from, destination_dir);
        unsupported()
    }

    #[cfg(windows)]
    {
        let source = paths::normalize(from)?;
        let destination = paths::normalize(destination_dir)?;
        if !fs::metadata(paths::to_verbatim(&destination)?)?.is_dir() {
            return Err(Error::InvalidArgument(format!(
                "`{}` is not a directory",
                paths::display(&destination)
            )));
        }
        let name = source
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| Error::InvalidPath {
                path: source.clone(),
                reason: "the object has no usable file name".to_owned(),
            })?;
        let target = paths::join_within_limit(&paths::strip_verbatim(&destination), name)?;
        let target_verbatim = paths::to_verbatim(&target)?;
        if target_verbatim.starts_with(&source) {
            return Err(Error::InvalidArgument(
                "an object cannot be copied into its own subtree".to_owned(),
            ));
        }
        ensure_absent(&source, &target)?;

        let bytes = copy_recursive(&source, &target)?;

        Ok(LongPathOutcome {
            operation: LongPathOperation::Copy,
            from: from.to_path_buf(),
            to: Some(target),
            bytes,
        })
    }
}

/// Deletes an object permanently, including read-only files.
///
/// # Errors
/// Returns [`Error::ProtectedPath`] for system locations and [`Error::Io`] for
/// failures. On non-Windows platforms it returns [`Error::Unsupported`].
pub fn delete(path: &Path) -> Result<LongPathOutcome> {
    #[cfg(not(windows))]
    {
        let _ = path;
        unsupported()
    }

    #[cfg(windows)]
    {
        if crate::confirm::is_protected_path(path) {
            return Err(Error::ProtectedPath(path.to_path_buf()));
        }
        let source = paths::normalize(path)?;
        let bytes = size_of(&source)?;
        remove_recursive(&paths::to_verbatim(&source)?)?;

        Ok(LongPathOutcome {
            operation: LongPathOperation::Delete,
            from: path.to_path_buf(),
            to: None,
            bytes,
        })
    }
}

#[cfg(windows)]
fn ensure_absent(source: &Path, target: &Path) -> Result<()> {
    let source_verbatim = paths::to_verbatim(source)?;
    if !source_verbatim.exists() {
        return Err(Error::NotFound(paths::display(source)));
    }
    let target_verbatim = paths::to_verbatim(target)?;
    if target_verbatim.exists() {
        return Err(Error::Conflict(format!(
            "`{}` already exists",
            paths::display(target)
        )));
    }

    Ok(())
}

#[cfg(windows)]
fn size_of(path: &Path) -> Result<u64> {
    let verbatim = paths::to_verbatim(path)?;
    let metadata = fs::symlink_metadata(&verbatim)?;
    if !metadata.is_dir() {
        return Ok(metadata.len());
    }

    let mut total = 0u64;
    let mut stack = vec![verbatim];
    while let Some(directory) = stack.pop() {
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if metadata.is_dir() {
                stack.push(entry.path());
            } else {
                total = total.saturating_add(metadata.len());
            }
        }
    }

    Ok(total)
}

#[cfg(windows)]
fn copy_recursive(from: &Path, to: &Path) -> Result<u64> {
    let from = paths::to_verbatim(from)?;
    let to = paths::to_verbatim(to)?;
    let metadata = fs::symlink_metadata(&from)?;

    if metadata.is_dir() {
        fs::create_dir_all(&to)?;
        let mut total = 0u64;
        for entry in fs::read_dir(&from)? {
            let entry = entry?;
            total =
                total.saturating_add(copy_recursive(&entry.path(), &to.join(entry.file_name()))?);
        }

        return Ok(total);
    }

    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(&from, &to)?;

    Ok(metadata.len())
}

#[cfg(windows)]
#[allow(clippy::permissions_set_readonly_false)]
fn remove_recursive(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;

    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            remove_recursive(&entry.path())?;
        }
        fs::remove_dir(path)?;

        return Ok(());
    }

    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
            let mut permissions = metadata.permissions();
            permissions.set_readonly(false);
            fs::set_permissions(path, permissions)?;
            fs::remove_file(path)?;

            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::{copy_to, delete, move_to, rename};
    use freespace_core::ErrorCode;

    #[test]
    fn renames_moves_copies_and_deletes_long_paths() {
        let directory = tempfile::tempdir().expect("temp dir");
        let mut current = directory.path().to_path_buf();
        for index in 0..8 {
            current.push(format!("segment-{index}-{}", "x".repeat(40)));
            std::fs::create_dir_all(
                freespace_platform::paths::to_verbatim(&current).expect("verbatim"),
            )
            .expect("create");
        }
        let source = current.join("payload.txt");
        std::fs::write(&source, b"payload").expect("write");
        assert!(source.as_os_str().len() > 270);

        let renamed = rename(&source, "renamed.txt").expect("rename");
        let renamed_path = renamed.to.expect("target");
        assert!(renamed_path.exists());
        assert!(!source.exists());

        let destination = directory.path().join("target");
        std::fs::create_dir_all(&destination).expect("target");
        let moved = move_to(&renamed_path, &destination).expect("move");
        let moved_path = moved.to.expect("target");
        assert!(moved_path.exists());

        let copied = copy_to(&moved_path, directory.path()).expect("copy");
        let copied_path = copied.to.expect("target");
        assert!(copied_path.exists());
        assert!(moved_path.exists());

        delete(&copied_path).expect("delete copy");
        delete(&moved_path).expect("delete moved");
        assert!(!copied_path.exists());
        assert!(!moved_path.exists());
    }

    #[test]
    fn refuses_overlong_names_before_changing_data() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("source.txt");
        std::fs::write(&path, b"payload").expect("write");

        let error = rename(
            &path,
            &"x".repeat(freespace_platform::paths::MAX_PATH_CHARS),
        )
        .expect_err("too long");

        assert_eq!(error.code(), ErrorCode::InvalidPath);
        assert!(path.exists());
    }

    #[test]
    fn refuses_conflicts_and_missing_sources() {
        let directory = tempfile::tempdir().expect("temp dir");
        let source = directory.path().join("source.txt");
        let existing = directory.path().join("existing.txt");
        std::fs::write(&source, b"one").expect("source");
        std::fs::write(&existing, b"two").expect("existing");

        let error = rename(&source, "existing.txt").expect_err("conflict");
        assert_eq!(error.code(), ErrorCode::Conflict);

        let error = delete(&directory.path().join("missing.txt")).expect_err("missing");
        assert_eq!(error.code(), ErrorCode::NotFound);
    }
}

#[cfg(all(test, unix))]
mod unix_tests {
    use super::rename;
    use freespace_core::ErrorCode;

    #[test]
    fn long_path_operations_are_unsupported_on_unix() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("source.txt");
        std::fs::write(&path, b"payload").expect("write");

        let error = rename(&path, "renamed.txt").expect_err("unsupported");

        assert_eq!(error.code(), ErrorCode::Unsupported);
        assert!(path.exists());
    }
}
