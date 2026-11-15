//! Thin, safe wrappers around the most common WinAPI primitives.
//!
//! The goal is not to re-export the whole `windows` crate but to give the rest
//! of the platform layer two reliable building blocks: an RAII handle and
//! WinAPI/`io::Error` conversion.

use std::{
    ffi::{OsStr, OsString, c_void},
    fmt, io,
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::Path,
};

use freespace_core::{Error, Result};
use windows::{
    Win32::{
        Foundation::{CloseHandle, GetLastError, HANDLE},
        Storage::FileSystem::{
            CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_SHARE_DELETE,
            FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
        },
    },
    core::PCWSTR,
};

/// Converts a string slice into a NUL-terminated UTF-16 buffer.
#[must_use]
pub fn to_wide(value: &OsStr) -> Vec<u16> {
    let mut wide: Vec<u16> = value.encode_wide().collect();
    wide.push(0);

    wide
}

/// Converts a NUL-terminated (or unterminated) UTF-16 buffer into an [`OsString`].
#[must_use]
pub fn from_wide(value: &[u16]) -> OsString {
    let end = value
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(value.len());

    OsString::from_wide(&value[..end])
}

/// Returns the thread's last Win32 error as an [`io::Error`].
#[must_use]
pub fn last_error() -> io::Error {
    // SAFETY: GetLastError has no preconditions and does not touch memory.
    let code = unsafe { GetLastError() };

    io::Error::from_raw_os_error(code.0 as i32)
}

/// Converts a `windows` crate error into an [`io::Error`].
///
/// WinAPI failures arrive as `HRESULT_FROM_WIN32` (`0x8007xxxx`); unwrapping the
/// low word restores the Win32 code, which preserves [`io::ErrorKind`] mapping
/// (not found, access denied, ...).
#[must_use]
pub fn win_error(error: &windows::core::Error) -> io::Error {
    let hresult = error.code().0 as u32;
    let code = if hresult & 0xFFFF_0000 == 0x8007_0000 {
        (hresult & 0xFFFF) as i32
    } else {
        error.code().0
    };

    io::Error::from_raw_os_error(code)
}

/// An owned WinAPI handle that is closed on drop.
pub struct Handle(HANDLE);

impl Handle {
    /// Wraps a raw handle, rejecting `NULL` and `INVALID_HANDLE_VALUE`.
    ///
    /// # Errors
    /// Returns [`Error::Io`] with the thread's last error when the handle is
    /// invalid.
    pub fn from_raw(handle: HANDLE) -> Result<Self> {
        if handle.is_invalid() {
            return Err(Error::Io(last_error()));
        }

        Ok(Self(handle))
    }

    /// Opens an existing file or directory for reading.
    ///
    /// The path is converted to verbatim form, so the call works for paths
    /// longer than `MAX_PATH`.
    ///
    /// # Errors
    /// Returns [`Error::InvalidPath`] for unusable paths and [`Error::Io`] when
    /// the object cannot be opened.
    pub fn open_read(path: &Path) -> Result<Self> {
        let verbatim = super::paths::to_verbatim(path)?;
        let wide = to_wide(verbatim.as_os_str());

        // SAFETY: `wide` is NUL-terminated and lives for the duration of the call.
        let handle = unsafe {
            CreateFileW(
                PCWSTR(wide.as_ptr()),
                FILE_GENERIC_READ.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        }
        .map_err(|error| Error::Io(win_error(&error)))?;

        Self::from_raw(handle)
    }

    /// Returns the raw handle value.
    #[must_use]
    pub const fn raw(&self) -> HANDLE {
        self.0
    }

    /// Returns `true` when the handle is `NULL` or `INVALID_HANDLE_VALUE`.
    #[must_use]
    pub fn is_invalid(&self) -> bool {
        self.0.is_invalid()
    }
}

impl fmt::Debug for Handle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Handle")
            .field(&(self.0.0 as *const c_void))
            .finish()
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY: the handle is owned by this value and closed exactly once.
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsStr, io};

    use freespace_core::ErrorCode;

    use super::{Handle, from_wide, to_wide};

    #[test]
    fn round_trips_unicode_wide_strings() {
        let value = OsStr::new(r"C:\Данные\файл.txt");
        let wide = to_wide(value);

        assert_eq!(wide.last(), Some(&0));
        assert_eq!(from_wide(&wide), value);
    }

    #[test]
    fn trims_at_the_first_terminator() {
        let wide = [b'a' as u16, b'b' as u16, 0, b'c' as u16];

        assert_eq!(from_wide(&wide), OsStr::new("ab"));
    }

    #[test]
    fn opens_and_closes_a_file() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("payload.bin");
        std::fs::write(&path, b"payload").expect("write fixture");

        let handle = Handle::open_read(&path).expect("open file");

        assert!(!handle.is_invalid());
        drop(handle);
    }

    #[test]
    fn maps_missing_files_to_not_found() {
        let directory = tempfile::tempdir().expect("temp dir");
        let error = Handle::open_read(&directory.path().join("missing.bin"))
            .expect_err("missing file must fail");

        assert_eq!(error.code(), ErrorCode::NotFound);
        match error {
            freespace_core::Error::Io(io_error) => {
                assert_eq!(io_error.kind(), io::ErrorKind::NotFound);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
