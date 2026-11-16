//! Reparse-point inspection (FR-047, FR-048).
//!
//! A junction is a directory reparse point, a symbolic link can point to a file
//! or a directory, and other tags exist for cloud placeholders and deduplication.
//! The scanner must know the difference: links are never traversed implicitly and
//! their targets are never counted twice.

use std::{
    fs, io,
    os::windows::fs::MetadataExt,
    path::{Path, PathBuf},
};

use freespace_core::{Error, Result};
use windows::{
    Win32::{
        Storage::FileSystem::{
            CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_BACKUP_SEMANTICS,
            FILE_FLAG_OPEN_REPARSE_POINT, FILE_GENERIC_READ, FILE_SHARE_DELETE, FILE_SHARE_READ,
            FILE_SHARE_WRITE, OPEN_EXISTING,
        },
        System::{IO::DeviceIoControl, Ioctl::FSCTL_GET_REPARSE_POINT},
    },
    core::PCWSTR,
};

use super::{paths, win};

/// `IO_REPARSE_TAG_MOUNT_POINT`.
pub const TAG_MOUNT_POINT: u32 = 0xA000_0003;
/// `IO_REPARSE_TAG_SYMLINK`.
pub const TAG_SYMLINK: u32 = 0xA000_000C;

/// Kind of a reparse point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReparseKind {
    /// Symbolic link.
    Symlink,
    /// NTFS junction (directory mount point).
    Junction,
    /// Another reparse tag (cloud placeholder, deduplication, ...).
    Other,
}

impl ReparseKind {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Symlink => "symlink",
            Self::Junction => "junction",
            Self::Other => "other",
        }
    }
}

/// Details of one reparse point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReparseInfo {
    /// Reparse tag.
    pub tag: u32,
    /// Classified kind.
    pub kind: ReparseKind,
    /// Target path when it could be read.
    pub target: Option<PathBuf>,
}

/// Inspects `path`, returning `None` when it is not a reparse point.
///
/// # Errors
/// Returns [`Error::NotFound`] for a missing path and [`Error::Io`] when the
/// reparse data cannot be read.
pub fn inspect(path: &Path) -> Result<Option<ReparseInfo>> {
    let verbatim = paths::to_verbatim(path)?;
    let metadata = fs::symlink_metadata(&verbatim).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            Error::NotFound(paths::display(path))
        } else {
            Error::Io(error)
        }
    })?;
    // FILE_ATTRIBUTE_REPARSE_POINT
    if metadata.file_attributes() & 0x400 == 0 {
        return Ok(None);
    }

    let tag = reparse_tag(&verbatim)?;
    let kind = match tag {
        TAG_MOUNT_POINT => ReparseKind::Junction,
        TAG_SYMLINK => ReparseKind::Symlink,
        _ => ReparseKind::Other,
    };
    // Junction targets are not always reported by `read_link`; the raw tag is
    // still enough to classify the object.
    let target = fs::read_link(&verbatim).ok();

    Ok(Some(ReparseInfo { tag, kind, target }))
}

fn reparse_tag(path: &Path) -> Result<u32> {
    let wide = win::to_wide(path.as_os_str());
    // Directories need FILE_FLAG_BACKUP_SEMANTICS to be opened.
    // SAFETY: `wide` is NUL-terminated and lives for the call.
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            FILE_GENERIC_READ.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            None,
        )
    }
    .map_err(|error| Error::Io(win::win_error(&error)))?;
    let handle = win::Handle::from_raw(handle)?;

    let mut buffer = vec![0u8; 16 * 1024];
    let mut returned = 0u32;

    // SAFETY: the output buffer is valid for the call and the size matches.
    unsafe {
        DeviceIoControl(
            handle.raw(),
            FSCTL_GET_REPARSE_POINT,
            None,
            0,
            Some(buffer.as_mut_ptr().cast()),
            u32::try_from(buffer.len()).unwrap_or(0),
            Some(&mut returned),
            None,
        )
    }
    .map_err(|error| Error::Io(win::win_error(&error)))?;
    if returned < 4 {
        return Err(Error::Io(io::Error::other(
            "the reparse point returned no tag",
        )));
    }

    Ok(u32::from_le_bytes(buffer[..4].try_into().map_err(
        |_| Error::Io(io::Error::other("short reparse tag")),
    )?))
}

/// Returns `true` when the object is a reparse point.
///
/// # Errors
/// Returns an error when metadata cannot be read.
pub fn is_reparse_point(path: &Path) -> Result<bool> {
    let verbatim = paths::to_verbatim(path)?;
    let metadata = fs::symlink_metadata(&verbatim)?;

    Ok(metadata.file_attributes() & 0x400 != 0)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{ReparseKind, inspect, is_reparse_point};

    #[test]
    fn plain_files_are_not_reparse_points() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("plain.txt");
        std::fs::write(&path, b"payload").expect("write");

        assert_eq!(inspect(&path).expect("inspect"), None);
        assert!(!is_reparse_point(&path).expect("check"));
    }

    #[test]
    fn detects_a_symlink_when_the_environment_allows_it() {
        let directory = tempfile::tempdir().expect("temp dir");
        let target = directory.path().join("target.txt");
        std::fs::write(&target, b"payload").expect("write");
        let link = directory.path().join("link.txt");
        if std::os::windows::fs::symlink_file(&target, &link).is_err() {
            return;
        }

        let info = inspect(&link).expect("inspect").expect("a reparse point");

        assert_eq!(info.kind, ReparseKind::Symlink);
        assert_eq!(info.tag, super::TAG_SYMLINK);
        assert_eq!(
            info.target.as_deref().and_then(Path::file_name),
            Some(std::ffi::OsStr::new("target.txt"))
        );
    }
}
