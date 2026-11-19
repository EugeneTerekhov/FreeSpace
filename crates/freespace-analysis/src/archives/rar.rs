//! Optional RAR reader (FR-066).
//!
//! RAR support depends on a native `unrar` library. To keep the default build
//! self-contained the reader is behind the `rar` Cargo feature; without it the
//! command reports the documented `E_UNSUPPORTED` error instead of failing to
//! build or crashing.

use std::path::Path;

use freespace_core::{Error, Result};

use super::ArchiveEntry;

/// Lists a RAR archive when the optional reader is available.
///
/// # Errors
/// Returns [`Error::Unsupported`] when the feature is disabled, the native
/// library is missing or the archive is password-protected, and
/// [`Error::Operation`] when the archive is damaged.
pub fn read_rar(path: &Path) -> Result<Vec<ArchiveEntry>> {
    #[cfg(not(feature = "rar"))]
    {
        let _ = path;

        Err(Error::Unsupported(
            "RAR support is not compiled in; rebuild with the `rar` feature and a compatible \
             unrar library"
                .to_owned(),
        ))
    }

    #[cfg(feature = "rar")]
    {
        read_with_unrar(path)
    }
}

#[cfg(feature = "rar")]
fn read_with_unrar(path: &Path) -> Result<Vec<ArchiveEntry>> {
    let listing = unrar::Archive::new(path)
        .open_for_listing()
        .map_err(|error| map_error(&error.to_string()))?;
    let mut entries = Vec::new();

    for header in listing {
        let header = header.map_err(|error| map_error(&error.to_string()))?;
        let internal_path = header.filename.to_string_lossy().into_owned();
        if header.is_directory() {
            entries.push(ArchiveEntry::directory(internal_path));
        } else {
            let mut entry = ArchiveEntry::file(internal_path, header.unpacked_size, None);
            entry.encrypted = header.is_encrypted();
            entries.push(entry);
        }
    }

    Ok(entries)
}

#[cfg(feature = "rar")]
fn map_error(message: &str) -> Error {
    let lowered = message.to_ascii_lowercase();
    if lowered.contains("password") || lowered.contains("encrypt") {
        Error::Unsupported(format!("the RAR archive is password-protected: {message}"))
    } else if lowered.contains("library") || lowered.contains("dll") {
        Error::Unsupported(format!("the RAR library is unavailable: {message}"))
    } else {
        Error::Operation(format!("corrupted RAR archive: {message}"))
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::read_rar;

    #[test]
    fn reports_missing_optional_support() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("sample.rar");
        std::fs::write(&path, b"Rar!\x1A\x07").expect("write");

        let result = read_rar(Path::new(&path));

        #[cfg(not(feature = "rar"))]
        assert_eq!(
            result.expect_err("feature disabled").code().as_str(),
            "E_UNSUPPORTED"
        );
        #[cfg(feature = "rar")]
        assert!(result.is_err(), "a truncated RAR must not parse");
    }
}
