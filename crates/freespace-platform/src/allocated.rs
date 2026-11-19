//! Exact on-disk allocation for compressed and sparse files (FR-049, FR-050).
//!
//! `GetCompressedFileSizeW` returns the space actually allocated to a file, which
//! is smaller than the logical length for compressed and sparse objects.
//! `FSCTL_QUERY_ALLOCATED_RANGES` exposes the allocated ranges themselves, used
//! for progress reporting and for tests of sparse files.

use std::{io, ops::Range, path::Path};

use freespace_core::{Error, Result};
use windows::{
    Win32::{
        Foundation::GetLastError,
        Storage::FileSystem::GetCompressedFileSizeW,
        System::{
            IO::DeviceIoControl,
            Ioctl::{FILE_ALLOCATED_RANGE_BUFFER, FSCTL_QUERY_ALLOCATED_RANGES},
        },
    },
    core::PCWSTR,
};

use super::{paths, win};

/// `ERROR_MORE_DATA`.
const ERROR_MORE_DATA: i32 = 234;
/// Number of ranges requested in one device-control call.
const RANGE_BATCH: usize = 256;

/// Returns the space allocated to `path` on disk.
///
/// # Errors
/// Returns [`Error::NotFound`] for a missing object and [`Error::Io`] when the
/// query fails.
pub fn allocated_size(path: &Path) -> Result<u64> {
    let verbatim = paths::to_verbatim(path)?;
    let wide = win::to_wide(verbatim.as_os_str());
    let mut high = 0u32;

    // SAFETY: `wide` is NUL-terminated and `high` is a valid out-pointer.
    let low = unsafe { GetCompressedFileSizeW(PCWSTR(wide.as_ptr()), Some(&mut high)) };
    if low == u32::MAX {
        // SAFETY: GetLastError has no preconditions.
        let code = unsafe { GetLastError() };
        if code.0 != 0 {
            let error = io::Error::from_raw_os_error(code.0 as i32);
            return Err(if error.kind() == io::ErrorKind::NotFound {
                Error::NotFound(paths::display(path))
            } else {
                Error::Io(error)
            });
        }
    }

    Ok((u64::from(high) << 32) | u64::from(low))
}

/// Returns the byte ranges that are actually allocated on disk.
///
/// Empty and fully sparse files produce an empty list.
///
/// # Errors
/// Returns [`Error::Unsupported`] when the file system cannot answer the query
/// and [`Error::Io`] for other failures.
pub fn allocated_ranges(path: &Path) -> Result<Vec<Range<u64>>> {
    let handle = win::Handle::open_read(path)?;
    let mut ranges = Vec::new();
    let mut offset = 0u64;

    loop {
        let input = FILE_ALLOCATED_RANGE_BUFFER {
            FileOffset: i64::try_from(offset).unwrap_or(i64::MAX),
            Length: i64::try_from(u64::MAX.saturating_sub(offset)).unwrap_or(i64::MAX),
        };
        let mut output = vec![FILE_ALLOCATED_RANGE_BUFFER::default(); RANGE_BATCH];
        let mut returned = 0u32;

        // SAFETY: both buffers live for the call and their sizes match the
        // corresponding type.
        let result = unsafe {
            DeviceIoControl(
                handle.raw(),
                FSCTL_QUERY_ALLOCATED_RANGES,
                Some(std::ptr::from_ref(&input).cast()),
                u32::try_from(size_of_val(&input)).unwrap_or(0),
                Some(output.as_mut_ptr().cast()),
                u32::try_from(size_of_val(&output)).unwrap_or(0),
                Some(&mut returned),
                None,
            )
        };

        let count = usize::try_from(returned)
            .unwrap_or(0)
            .saturating_div(size_of::<FILE_ALLOCATED_RANGE_BUFFER>())
            .min(output.len());
        for entry in &output[..count] {
            let start = u64::try_from(entry.FileOffset).unwrap_or(0);
            let length = u64::try_from(entry.Length).unwrap_or(0);
            if length > 0 {
                ranges.push(start..start.saturating_add(length));
                offset = start.saturating_add(length);
            }
        }

        match result {
            Ok(()) => break,
            Err(error) => {
                let io_error = win::win_error(&error);
                if io_error.raw_os_error() == Some(ERROR_MORE_DATA) && count > 0 {
                    continue;
                }
                return Err(Error::Unsupported(format!(
                    "allocated ranges are not available: {io_error}"
                )));
            }
        }
    }

    Ok(ranges)
}

#[cfg(test)]
mod tests {
    use super::{allocated_ranges, allocated_size};

    #[test]
    fn reports_allocated_size_for_a_plain_file() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("payload.bin");
        std::fs::write(&path, vec![0xAB; 8_192]).expect("write");

        let allocated = allocated_size(&path).expect("allocated size");

        assert!(allocated >= 8_192);
        assert!(allocated <= 64 * 1_024);
    }

    #[test]
    fn reports_allocated_ranges_or_reports_unsupported() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("payload.bin");
        std::fs::write(&path, vec![0x5A; 16_384]).expect("write");

        match allocated_ranges(&path) {
            Ok(ranges) => {
                assert!(!ranges.is_empty());
                assert!(ranges.iter().all(|range| range.start < range.end));
            }
            Err(error) => assert_eq!(error.code().as_str(), "E_UNSUPPORTED"),
        }
    }

    #[test]
    fn reports_missing_files() {
        let directory = tempfile::tempdir().expect("temp dir");
        let error = allocated_size(&directory.path().join("missing.bin")).expect_err("missing");

        assert_eq!(error.code().as_str(), "E_NOT_FOUND");
    }
}
