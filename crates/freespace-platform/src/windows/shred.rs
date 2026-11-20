//! Windows implementation of the shredder primitive.
//!
//! The operations layer owns confirmation, protected-path checks and journal
//! recording; this module only overwrites the file, flushes every pass with
//! write-through semantics, asks the file system to TRIM the clusters and
//! removes the file under a random name.

use std::{
    fs,
    io::{Seek, SeekFrom, Write},
    path::Path,
};

use freespace_core::{Error, Result};

use super::{paths, trim, win};
use crate::common::{
    shred::{ShredOptions, ShredOutcome, ShredPattern, ShredPreview},
    storage::StorageKind,
};

/// Size of the overwrite buffer.
const BUFFER: usize = 1024 * 1024;

/// Warning shown for solid-state media.
pub const SSD_WARNING: &str = "solid-state drive: overwriting cannot guarantee destruction because wear levelling may keep physical copies";

/// Validates the parameters.
///
/// # Errors
/// Returns [`Error::InvalidArgument`] when no overwrite pass was requested.
pub fn validate(options: &ShredOptions) -> Result<()> {
    if options.passes == 0 {
        return Err(Error::InvalidArgument(
            "the shredder needs at least one overwrite pass".to_owned(),
        ));
    }

    Ok(())
}

/// Describes what shredding `path` would do.
///
/// # Errors
/// Returns [`Error::NotFound`] for a missing object, [`Error::InvalidArgument`]
/// for a directory and [`Error::Io`] when metadata cannot be read.
pub fn preview(path: &Path) -> Result<ShredPreview> {
    let absolute = absolute_path(path)?;
    let metadata = metadata_for(&absolute)?;
    if metadata.is_dir() {
        return Err(Error::InvalidArgument(format!(
            "`{}` is a directory; the shredder only destroys files",
            paths::display(&absolute)
        )));
    }

    let storage = trim::storage_kind(&absolute).unwrap_or(StorageKind::Unknown);
    let mut warnings = Vec::new();
    if storage.needs_warning() {
        warnings.push(SSD_WARNING.to_owned());
    }

    Ok(ShredPreview {
        path: absolute,
        size: metadata.len(),
        storage,
        warnings,
    })
}

/// Overwrites and removes `path`.
///
/// # Errors
/// Returns [`Error::InvalidArgument`] for invalid options or a directory, and
/// [`Error::Io`] when a step fails.
pub fn run(path: &Path, options: ShredOptions) -> Result<ShredOutcome> {
    validate(&options)?;

    let absolute = absolute_path(path)?;
    let verbatim = paths::to_verbatim(&absolute)?;
    let metadata = metadata_for(&absolute)?;
    if metadata.is_dir() {
        return Err(Error::InvalidArgument(format!(
            "`{}` is a directory; the shredder only destroys files",
            paths::display(&absolute)
        )));
    }

    let size = metadata.len();
    let storage = trim::storage_kind(&absolute).unwrap_or(StorageKind::Unknown);
    let mut warnings = Vec::new();
    if storage.needs_warning() {
        warnings.push(SSD_WARNING.to_owned());
    }

    if metadata.file_type().is_symlink() {
        // Overwriting a link would modify the target; removing the link is the
        // documented behaviour.
        fs::remove_file(&verbatim).map_err(Error::Io)?;

        return Ok(ShredOutcome {
            path: absolute,
            size,
            passes: options.passes,
            pattern: options.pattern,
            storage,
            trimmed: false,
            warnings,
        });
    }

    make_writable(&verbatim, &metadata)?;

    let mut file = win::open_write_through(&absolute)?;
    for _ in 0..options.passes {
        file.seek(SeekFrom::Start(0)).map_err(Error::Io)?;
        write_pattern(&mut file, size, options.pattern)?;
        file.sync_all().map_err(Error::Io)?;
    }

    // The clusters are still allocated here, so a TRIM request actually has
    // something to discard; truncating first would make it a no-op.
    let trimmed = match trim::trim_file(&absolute) {
        Ok(()) => true,
        Err(error) => {
            warnings.push(error.to_string());
            false
        }
    };

    file.set_len(0).map_err(Error::Io)?;
    file.sync_all().map_err(Error::Io)?;
    drop(file);

    let parent = absolute.parent().ok_or_else(|| Error::InvalidPath {
        path: absolute.clone(),
        reason: "the file has no parent directory".to_owned(),
    })?;
    let renamed = parent.join(random_name());
    let renamed_verbatim = paths::to_verbatim(&renamed)?;
    fs::rename(&verbatim, &renamed_verbatim).map_err(Error::Io)?;
    fs::remove_file(&renamed_verbatim).map_err(Error::Io)?;

    Ok(ShredOutcome {
        path: absolute,
        size,
        passes: options.passes,
        pattern: options.pattern,
        storage,
        trimmed,
        warnings,
    })
}

fn absolute_path(path: &Path) -> Result<std::path::PathBuf> {
    std::path::absolute(path).map_err(|error| Error::InvalidPath {
        path: path.to_path_buf(),
        reason: error.to_string(),
    })
}

fn metadata_for(path: &Path) -> Result<fs::Metadata> {
    let verbatim = paths::to_verbatim(path)?;
    fs::symlink_metadata(&verbatim).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            Error::NotFound(paths::display(path))
        } else {
            Error::Io(error)
        }
    })
}

#[allow(clippy::permissions_set_readonly_false)]
fn make_writable(verbatim: &Path, metadata: &fs::Metadata) -> Result<()> {
    let mut permissions = metadata.permissions();
    if permissions.readonly() {
        permissions.set_readonly(false);
        fs::set_permissions(verbatim, permissions).map_err(Error::Io)?;
    }

    Ok(())
}

fn write_pattern(file: &mut fs::File, size: u64, pattern: ShredPattern) -> Result<()> {
    if size == 0 {
        return Ok(());
    }

    let chunk = usize::try_from(size.min(BUFFER as u64))
        .unwrap_or(BUFFER)
        .max(1);
    let mut buffer = vec![0u8; chunk];
    let mut remaining = size;

    while remaining > 0 {
        let wanted = usize::try_from(remaining.min(buffer.len() as u64)).unwrap_or(buffer.len());
        match pattern {
            ShredPattern::Zeros => buffer[..wanted].fill(0),
            ShredPattern::Random => getrandom::fill(&mut buffer[..wanted])
                .map_err(|error| Error::Other(format!("random generator failed: {error}")))?,
        }
        file.write_all(&buffer[..wanted]).map_err(Error::Io)?;
        remaining = remaining.saturating_sub(wanted as u64);
    }

    Ok(())
}

fn random_name() -> String {
    let mut bytes = [0u8; 16];
    if getrandom::fill(&mut bytes).is_err() {
        // Fall back to the clock; the name only has to be unpredictable enough
        // to defeat name-based recovery.
        let now = freespace_core::Timestamp::now().as_millis().to_le_bytes();
        let length = bytes.len().min(now.len());
        bytes[..length].copy_from_slice(&now[..length]);
    }

    let mut name = String::with_capacity(32);
    for byte in bytes {
        name.push(char::from(b"0123456789abcdef"[usize::from(byte >> 4)]));
        name.push(char::from(b"0123456789abcdef"[usize::from(byte & 0x0f)]));
    }

    name
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{preview, run};
    use crate::common::shred::{ShredOptions, ShredPattern};

    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("secret.bin");
        std::fs::write(&path, vec![0x42u8; 4096]).expect("write");

        (directory, path)
    }

    #[test]
    fn rejects_zero_passes() {
        let (_directory, path) = fixture();

        let error = run(
            &path,
            ShredOptions {
                passes: 0,
                pattern: ShredPattern::Zeros,
            },
        )
        .expect_err("must refuse");

        assert_eq!(error.code().as_str(), "E_INVALID_ARGUMENT");
        assert!(path.exists());
    }

    #[test]
    fn destroys_a_file_and_reports_its_history() {
        let (_directory, path) = fixture();

        let outcome = run(
            &path,
            ShredOptions {
                passes: 3,
                pattern: ShredPattern::Random,
            },
        )
        .expect("shred");

        assert!(!path.exists(), "the file must be gone");
        assert_eq!(outcome.size, 4096);
        assert_eq!(outcome.passes, 3);
        assert_eq!(outcome.pattern, ShredPattern::Random);
    }

    #[test]
    fn previews_size_and_storage() {
        let (_directory, path) = fixture();

        let preview = preview(&path).expect("preview");

        assert_eq!(preview.size, 4096);
        assert!(path.exists(), "a preview must not change the file");
    }

    #[test]
    fn rejects_directories() {
        let directory = tempfile::tempdir().expect("temp dir");

        let error = run(directory.path(), ShredOptions::default()).expect_err("dir");

        assert_eq!(error.code().as_str(), "E_INVALID_ARGUMENT");
    }
}
