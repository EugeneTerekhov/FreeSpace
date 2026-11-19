//! Irreversible destruction of files.
//!
//! The shredder overwrites the complete file with the selected pattern and a
//! configurable number of passes, flushes every pass with write-through
//! semantics, truncates the file and finally deletes it under a random name
//! (FR-030). On solid-state media the user is warned that wear levelling can
//! keep physical copies; the file system is then asked to TRIM the clusters
//! (FR-031). Every run is journalled with its method and pass count (FR-033).
//!
//! Only the platform crate touches WinAPI: this module consumes
//! [`freespace_platform::win::open_write_through`] and
//! [`freespace_platform::trim`].

use std::{
    fs,
    io::{Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use freespace_core::{Error, Result};
use freespace_platform::{
    paths,
    trim::{self, StorageKind},
    win,
};
use serde_json::json;

use crate::{
    confirm::{is_protected_path, require_confirmation},
    operation::{
        JournalEntry, Operation, OperationKind, OperationStatus, OperationTarget, ShredPattern,
        TargetStatus,
    },
    store::Store,
};

/// Size of the overwrite buffer: large enough to amortise syscalls, small
/// enough to stay in cache.
pub const BUFFER: usize = 1024 * 1024;

/// Warning shown for solid-state media.
pub const SSD_WARNING: &str = "solid-state drive: overwriting cannot guarantee destruction because wear levelling may keep physical copies";

/// Parameters of a shred operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShredOptions {
    /// Number of overwrite passes; must be at least one.
    pub passes: u32,
    /// Overwrite pattern.
    pub pattern: ShredPattern,
}

impl Default for ShredOptions {
    fn default() -> Self {
        Self {
            passes: 1,
            pattern: ShredPattern::Zeros,
        }
    }
}

impl ShredOptions {
    /// Validates the parameters.
    ///
    /// # Errors
    /// Returns [`Error::InvalidArgument`] when no overwrite pass was requested.
    pub fn validate(&self) -> Result<()> {
        if self.passes == 0 {
            return Err(Error::InvalidArgument(
                "the shredder needs at least one overwrite pass".to_owned(),
            ));
        }

        Ok(())
    }
}

/// Result of shredding one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShredOutcome {
    /// Absolute path that was destroyed.
    pub path: PathBuf,
    /// Size before the overwrite.
    pub size: u64,
    /// Applied number of passes.
    pub passes: u32,
    /// Applied pattern.
    pub pattern: ShredPattern,
    /// Detected media kind.
    pub storage: StorageKind,
    /// Whether the file system performed a TRIM request.
    pub trimmed: bool,
    /// Non-fatal notes, including SSD limitations and TRIM failures.
    pub warnings: Vec<String>,
}

/// Overwrites and removes `path`.
///
/// # Errors
/// Returns [`Error::ConfirmationRequired`] without `confirmed`,
/// [`Error::InvalidArgument`] for invalid options or a directory,
/// [`Error::ProtectedPath`] for system locations and [`Error::Io`] when a step
/// fails. The file is only touched after every guard passed.
pub fn shred(path: &Path, options: ShredOptions, confirmed: bool) -> Result<ShredOutcome> {
    require_confirmation(confirmed)?;
    options.validate()?;

    let absolute = std::path::absolute(path).map_err(|error| Error::InvalidPath {
        path: path.to_path_buf(),
        reason: error.to_string(),
    })?;
    if is_protected_path(&absolute) {
        return Err(Error::ProtectedPath(absolute));
    }

    let verbatim = paths::to_verbatim(&absolute)?;
    let metadata = fs::symlink_metadata(&verbatim).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            Error::NotFound(paths::display(&absolute))
        } else {
            Error::Io(error)
        }
    })?;
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

/// Appends the results of a shred run to the operation journal.
///
/// # Errors
/// Returns [`Error::InvalidArgument`] for an empty result list and an error when
/// the journal cannot be written.
pub fn record(store: &Store, outcomes: &[ShredOutcome], user: &str) -> Result<u64> {
    let first = outcomes
        .first()
        .ok_or_else(|| Error::InvalidArgument("nothing was shredded".to_owned()))?;

    let targets: Vec<OperationTarget> = outcomes
        .iter()
        .map(|outcome| OperationTarget::new(outcome.path.clone(), outcome.size))
        .collect();
    let mut operation = Operation::new(OperationKind::Shred, targets);
    operation.params.passes = Some(first.passes);
    operation.params.shred_pattern = Some(first.pattern);
    operation.confirm()?;
    operation.begin()?;
    for target in &mut operation.targets {
        target.status = TargetStatus::Succeeded;
    }
    operation.finish(OperationStatus::Completed)?;

    let mut entry = JournalEntry::from_operation(&operation, user);
    entry.summary = format!(
        "shred: {} files, {} passes, {}",
        outcomes.len(),
        first.passes,
        first.pattern.as_str()
    );
    entry.details = json!({
        "passes": first.passes,
        "pattern": first.pattern.as_str(),
        "targets": outcomes
            .iter()
            .map(|outcome| json!({
                "path": outcome.path.to_string_lossy(),
                "size": outcome.size,
                "storage": outcome.storage.as_str(),
                "trimmed": outcome.trimmed,
                "warnings": outcome.warnings,
            }))
            .collect::<Vec<_>>(),
    });

    store.journal_append(&entry)
}

// Windows-only application: clearing the read-only attribute is exactly the
// intended operation and has no POSIX "world writable" meaning here.
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
    use std::path::{Path, PathBuf};

    use super::{ShredOptions, record, shred};
    use crate::{operation::ShredPattern, store::Store};

    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("secret.bin");
        std::fs::write(&path, vec![0x42u8; 4096]).expect("write");

        (directory, path)
    }

    #[test]
    fn refuses_without_confirmation() {
        let (_directory, path) = fixture();

        let error = shred(&path, ShredOptions::default(), false).expect_err("must refuse");

        assert_eq!(error.code().as_str(), "E_CONFIRMATION_REQUIRED");
        assert!(
            path.exists(),
            "nothing may be destroyed without confirmation"
        );
    }

    #[test]
    fn rejects_zero_passes() {
        let (_directory, path) = fixture();

        let error = shred(
            &path,
            ShredOptions {
                passes: 0,
                pattern: ShredPattern::Zeros,
            },
            true,
        )
        .expect_err("must refuse");

        assert_eq!(error.code().as_str(), "E_INVALID_ARGUMENT");
        assert!(path.exists());
    }

    #[test]
    fn destroys_a_file_and_reports_its_history() {
        let (_directory, path) = fixture();

        let outcome = shred(
            &path,
            ShredOptions {
                passes: 3,
                pattern: ShredPattern::Random,
            },
            true,
        )
        .expect("shred");

        assert!(!path.exists(), "the file must be gone");
        assert_eq!(outcome.size, 4096);
        assert_eq!(outcome.passes, 3);
        assert_eq!(outcome.pattern, ShredPattern::Random);
        assert_eq!(path, outcome.path);
    }

    #[test]
    fn rejects_directories_and_protected_paths() {
        let directory = tempfile::tempdir().expect("temp dir");

        let error = shred(directory.path(), ShredOptions::default(), true).expect_err("dir");
        assert_eq!(error.code().as_str(), "E_INVALID_ARGUMENT");

        let error = shred(
            Path::new(r"C:\Windows\System32\drivers\etc\hosts"),
            ShredOptions::default(),
            true,
        )
        .expect_err("protected");
        assert_eq!(error.code().as_str(), "E_PROTECTED_PATH");
    }

    #[test]
    fn journals_the_run() {
        let (_directory, path) = fixture();
        let outcome = shred(
            &path,
            ShredOptions {
                passes: 2,
                pattern: ShredPattern::Zeros,
            },
            true,
        )
        .expect("shred");
        let store = Store::open_in_memory().expect("store");

        let id = record(&store, &[outcome], "tester").expect("record");
        let entries = store.journal_list(10).expect("list");

        assert!(id > 0);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].summary.starts_with("shred"));
        assert_eq!(entries[0].details["passes"], 2);
        assert_eq!(entries[0].details["pattern"], "zeros");
        assert!(entries[0].details["targets"][0]["path"].as_str().is_some());
        assert_eq!(entries[0].result, crate::operation::JournalResult::Success);
    }

    #[test]
    fn destroys_empty_files() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("empty.bin");
        std::fs::write(&path, []).expect("write");

        shred(&path, ShredOptions::default(), true).expect("shred");

        assert!(!path.exists());
    }
}
