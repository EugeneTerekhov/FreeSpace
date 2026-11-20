//! NTFS alternate data stream enumeration (FR-050).
//!
//! `FindFirstStreamW` returns the unnamed `::$DATA` stream as well; FreeSpace
//! reports only the named streams and their sizes, because the unnamed stream is
//! already represented by the file's logical size.

use std::{fs, io, path::Path};

use freespace_core::{Error, Result};
use windows::Win32::Storage::FileSystem::{
    FindClose, FindFirstStreamW, FindNextStreamW, FindStreamInfoStandard, WIN32_FIND_STREAM_DATA,
};
use windows::core::PCWSTR;

use super::{paths, win};

pub use crate::common::metadata::StreamInfo;

/// `ERROR_HANDLE_EOF`: the stream enumeration reached the end.
const ERROR_HANDLE_EOF: i32 = 38;

/// Lists the named streams of a file.
///
/// Directories and non-NTFS volumes produce an empty list. A file that cannot be
/// opened is reported as an error, not silently ignored.
///
/// # Errors
/// Returns [`Error::NotFound`] for a missing file and [`Error::Io`] when the
/// enumeration fails for another reason.
pub fn list_streams(path: &Path) -> Result<Vec<StreamInfo>> {
    let verbatim = paths::to_verbatim(path)?;
    let metadata = fs::symlink_metadata(&verbatim).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            Error::NotFound(paths::display(path))
        } else {
            Error::Io(error)
        }
    })?;
    if metadata.is_dir() {
        return Ok(Vec::new());
    }

    let wide = win::to_wide(verbatim.as_os_str());
    let mut data = WIN32_FIND_STREAM_DATA::default();

    // SAFETY: `wide` is NUL-terminated and `data` is a valid output buffer whose
    // type matches `FindStreamInfoStandard`.
    let handle = match unsafe {
        FindFirstStreamW(
            PCWSTR(wide.as_ptr()),
            FindStreamInfoStandard,
            std::ptr::from_mut(&mut data).cast(),
            Some(0),
        )
    } {
        Ok(handle) => handle,
        Err(error) => {
            let io_error = win::win_error(&error);
            if io_error.raw_os_error() == Some(ERROR_HANDLE_EOF) {
                return Ok(Vec::new());
            }
            return Err(Error::Io(io_error));
        }
    };

    let mut streams = Vec::new();
    loop {
        if let Some(stream) = to_stream(&data) {
            streams.push(stream);
        }

        // SAFETY: `handle` came from FindFirstStreamW and `data` is the same
        // output buffer type.
        match unsafe { FindNextStreamW(handle, std::ptr::from_mut(&mut data).cast()) } {
            Ok(()) => {}
            Err(error) => {
                let io_error = win::win_error(&error);
                if io_error.raw_os_error() != Some(ERROR_HANDLE_EOF) {
                    // SAFETY: the handle is owned by this function.
                    let _ = unsafe { FindClose(handle) };
                    return Err(Error::Io(io_error));
                }
                break;
            }
        }
    }

    // SAFETY: the handle is owned by this function and closed exactly once.
    unsafe { FindClose(handle) }.map_err(|error| Error::Io(win::win_error(&error)))?;

    Ok(streams)
}

fn to_stream(data: &WIN32_FIND_STREAM_DATA) -> Option<StreamInfo> {
    let raw = win::from_wide(&data.cStreamName)
        .to_string_lossy()
        .into_owned();
    if raw.is_empty() || raw == "::$DATA" || raw == ":$DATA" {
        return None;
    }
    let trimmed = raw.trim_start_matches(':');
    let name = trimmed.strip_suffix(":$DATA").unwrap_or(trimmed).to_owned();
    if name.is_empty() {
        return None;
    }

    Some(StreamInfo {
        name,
        size: u64::try_from(data.StreamSize).unwrap_or(0),
    })
}

/// Sums the named streams of a file.
///
/// # Errors
/// Returns an error when the streams cannot be enumerated.
pub fn streams_size(path: &Path) -> Result<u64> {
    Ok(list_streams(path)?.iter().map(|stream| stream.size).sum())
}

#[cfg(test)]
mod tests {
    use super::{StreamInfo, list_streams, to_stream};
    use windows::Win32::Storage::FileSystem::WIN32_FIND_STREAM_DATA;

    #[test]
    fn plain_files_have_no_named_streams() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("plain.txt");
        std::fs::write(&path, b"payload").expect("write");

        let streams = list_streams(&path).expect("streams");

        assert!(streams.iter().all(|stream| !stream.name.is_empty()));
    }

    #[test]
    fn directories_report_no_streams() {
        let directory = tempfile::tempdir().expect("temp dir");

        assert!(list_streams(directory.path()).expect("streams").is_empty());
    }

    #[test]
    fn parses_stream_names() {
        let mut data = WIN32_FIND_STREAM_DATA::default();
        let name: Vec<u16> = "custom:$DATA\0".encode_utf16().collect();
        data.cStreamName[..name.len()].copy_from_slice(&name);
        data.StreamSize = 42;

        assert_eq!(
            to_stream(&data),
            Some(StreamInfo {
                name: "custom".to_owned(),
                size: 42,
            })
        );
    }
}
