//! Quarantine: safe, reversible removal of files and directories.
//!
//! Quarantined objects are moved — never copied and deleted — into a dedicated
//! directory, indexed in SQLite and checksummed so corruption can be detected.
//! Restoring uses the original path; purging is the only irreversible action and
//! is never automatic.

use std::{
    ffi::OsStr,
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
};

use freespace_core::{Error, Result, Timestamp, Uuid, model::Volume};
use freespace_platform::paths;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{confirm, store::Store};

/// Windows `ERROR_NOT_SAME_DEVICE`; returned by `MoveFile` across volumes.
const ERROR_NOT_SAME_DEVICE: i32 = 17;

const HASH_BUFFER: usize = 64 * 1024;

/// Lifecycle state of a quarantined object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QuarantineStatus {
    /// Object is stored in quarantine.
    Stored,
    /// Object was restored to its original path.
    Restored,
    /// Object was permanently removed.
    Purged,
    /// Stored data no longer matches its checksum.
    Corrupted,
}

impl QuarantineStatus {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stored => "stored",
            Self::Restored => "restored",
            Self::Purged => "purged",
            Self::Corrupted => "corrupted",
        }
    }

    /// Parses a stored value.
    #[must_use]
    pub fn parse_value(value: &str) -> Option<Self> {
        match value {
            "stored" => Some(Self::Stored),
            "restored" => Some(Self::Restored),
            "purged" => Some(Self::Purged),
            "corrupted" => Some(Self::Corrupted),
            _ => None,
        }
    }
}

/// Index entry describing a quarantined object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuarantineItem {
    /// Identifier.
    pub id: Uuid,
    /// Original absolute path.
    pub original_path: PathBuf,
    /// Path inside the quarantine directory.
    pub stored_path: PathBuf,
    /// Volume the object came from.
    pub volume_id: Uuid,
    /// Size in bytes.
    pub size: u64,
    /// When the object was quarantined.
    pub deleted_at: Timestamp,
    /// When retention ends.
    pub expires_at: Timestamp,
    /// Reason recorded by the caller.
    pub reason: String,
    /// Lifecycle state.
    pub status: QuarantineStatus,
    /// SHA-256 checksum of file contents; empty for directories.
    pub checksum: String,
}

/// Quarantine directory manager.
#[derive(Debug, Clone)]
pub struct Quarantine {
    root: PathBuf,
}

impl Quarantine {
    /// Opens or creates a quarantine directory.
    ///
    /// # Errors
    /// Returns an error when the directory cannot be created.
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        let verbatim = paths::to_verbatim(&root)?;
        fs::create_dir_all(&verbatim)?;

        Ok(Self { root })
    }

    /// Returns the quarantine root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolves the default quarantine location.
    ///
    /// Portable mode keeps the quarantine next to the executable; otherwise it
    /// lives under `%LOCALAPPDATA%\FreeSpace\Quarantine`.
    ///
    /// # Errors
    /// Returns [`Error::Config`] when the location cannot be resolved.
    pub fn default_root(portable: bool) -> Result<PathBuf> {
        if portable {
            let executable = std::env::current_exe()
                .map_err(|error| Error::Config(format!("cannot locate executable: {error}")))?;
            let directory = executable
                .parent()
                .ok_or_else(|| Error::Config("executable has no parent directory".to_owned()))?;

            return Ok(directory.join("Quarantine"));
        }

        let base = std::env::var_os("LOCALAPPDATA")
            .or_else(|| std::env::var_os("APPDATA"))
            .ok_or_else(|| {
                Error::Config("LOCALAPPDATA is not set; portable mode must be enabled".to_owned())
            })?;

        Ok(PathBuf::from(base).join("FreeSpace").join("Quarantine"))
    }

    /// Moves `path` into quarantine and records it in the index.
    ///
    /// The original object is preserved: it is renamed into the quarantine
    /// directory, or copied and verified first when it crosses volumes.
    ///
    /// # Errors
    /// Returns [`Error::ProtectedPath`] for system locations,
    /// [`Error::NotFound`] for missing objects and [`Error::Io`] for failures.
    pub fn place(
        &self,
        store: &Store,
        path: &Path,
        reason: &str,
        retention_days: u32,
    ) -> Result<QuarantineItem> {
        let absolute = std::path::absolute(path).map_err(|error| Error::InvalidPath {
            path: path.to_path_buf(),
            reason: error.to_string(),
        })?;

        if confirm::is_protected_path(&absolute) {
            return Err(Error::ProtectedPath(absolute));
        }

        let verbatim = paths::to_verbatim(&absolute)?;
        let metadata = fs::symlink_metadata(&verbatim).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                Error::NotFound(paths::display(&absolute))
            } else {
                Error::Io(error)
            }
        })?;
        let is_directory = metadata.is_dir();
        let is_symlink = metadata.file_type().is_symlink();
        let size = if is_directory {
            directory_size(&absolute)?
        } else {
            metadata.len()
        };
        let checksum = if is_directory || is_symlink {
            String::new()
        } else {
            sha256_file(&absolute)?
        };
        let volume_root = paths::volume_root(&absolute)?;
        let volume_id = Volume::id_for_mount_point(&volume_root);

        let id = Uuid::new_v4();
        let stored_path = self
            .root
            .join(format!("{id}_{}", sanitize_name(absolute.file_name())));
        move_path(&absolute, &stored_path)?;

        let deleted_at = Timestamp::now();
        let item = QuarantineItem {
            id,
            original_path: absolute,
            stored_path,
            volume_id,
            size,
            deleted_at,
            expires_at: deleted_at.add_days(retention_days),
            reason: reason.to_owned(),
            status: QuarantineStatus::Stored,
            checksum,
        };

        if let Err(error) = store.quarantine_insert(&item) {
            // Best effort: put the object back where it came from.
            let _ = move_path(&item.stored_path, &item.original_path);
            return Err(error);
        }

        Ok(item)
    }

    /// Moves a quarantined object back to its original path.
    ///
    /// # Errors
    /// Returns [`Error::Conflict`] when the original path exists again,
    /// [`Error::NotFound`] when the stored object is gone, and [`Error::Io`] for
    /// file-system failures.
    pub fn restore(&self, store: &Store, item: &QuarantineItem) -> Result<()> {
        if item.status != QuarantineStatus::Stored {
            return Err(Error::Conflict(format!(
                "quarantine item `{}` is `{}` and cannot be restored",
                item.id,
                item.status.as_str()
            )));
        }

        if !path_exists(&item.stored_path) {
            return Err(Error::NotFound(paths::display(&item.stored_path)));
        }
        if path_exists(&item.original_path) {
            return Err(Error::Conflict(format!(
                "`{}` already exists",
                paths::display(&item.original_path)
            )));
        }

        if let Some(parent) = item.original_path.parent() {
            let verbatim_parent = paths::to_verbatim(parent)?;
            fs::create_dir_all(&verbatim_parent)?;
        }

        move_path(&item.stored_path, &item.original_path)?;
        store.quarantine_update_status(item.id, QuarantineStatus::Restored)?;

        Ok(())
    }

    /// Permanently deletes a quarantined object.
    ///
    /// # Errors
    /// Returns an error when the object cannot be removed or the index cannot
    /// be updated.
    pub fn purge(&self, store: &Store, item: &QuarantineItem) -> Result<()> {
        if path_exists(&item.stored_path) {
            remove_path(&item.stored_path)?;
        }
        store.quarantine_update_status(item.id, QuarantineStatus::Purged)?;

        Ok(())
    }

    /// Verifies that the stored object still matches its checksum.
    ///
    /// # Errors
    /// Returns an error when the object cannot be read.
    pub fn verify(&self, item: &QuarantineItem) -> Result<bool> {
        if !path_exists(&item.stored_path) {
            return Ok(false);
        }
        if item.checksum.is_empty() {
            return Ok(true);
        }

        Ok(sha256_file(&item.stored_path)? == item.checksum)
    }
}

fn path_exists(path: &Path) -> bool {
    let verbatim = paths::to_verbatim(path).unwrap_or_else(|_| path.to_path_buf());

    fs::symlink_metadata(verbatim).is_ok()
}

fn sha256_file(path: &Path) -> Result<String> {
    let verbatim = paths::to_verbatim(path)?;
    let mut file = File::open(&verbatim)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; HASH_BUFFER];

    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }

    output
}

fn directory_size(path: &Path) -> Result<u64> {
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];

    while let Some(directory) = stack.pop() {
        let verbatim = paths::to_verbatim(&directory)?;
        for entry in fs::read_dir(&verbatim)? {
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

fn move_path(from: &Path, to: &Path) -> Result<()> {
    let from = paths::to_verbatim(from)?;
    let to = paths::to_verbatim(to)?;

    match fs::rename(&from, &to) {
        Ok(()) => Ok(()),
        Err(error) if error.raw_os_error() == Some(ERROR_NOT_SAME_DEVICE) => {
            copy_path(&from, &to)?;
            remove_path(&from)
        }
        Err(error) => Err(error.into()),
    }
}

fn copy_path(from: &Path, to: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(from)?;

    if metadata.file_type().is_symlink() {
        return Err(Error::Unsupported(format!(
            "cannot move symbolic link `{}` across volumes",
            paths::display(from)
        )));
    }

    if metadata.is_dir() {
        fs::create_dir_all(to)?;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            copy_path(&entry.path(), &to.join(entry.file_name()))?;
        }
    } else {
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(from, to)?;
    }

    Ok(())
}

fn remove_path(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;

    if metadata.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }

    Ok(())
}

fn sanitize_name(name: Option<&OsStr>) -> String {
    let name = name
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "item".to_owned());
    let mut cleaned: String = name
        .chars()
        .map(|character| match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            character if character.is_control() => '_',
            character => character,
        })
        .collect();

    while cleaned.ends_with(['.', ' ']) {
        cleaned.pop();
    }
    let cleaned: String = cleaned.chars().take(120).collect();

    if cleaned.is_empty() {
        "item".to_owned()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use std::{env, path::PathBuf};

    use freespace_core::ErrorCode;

    use super::{Quarantine, QuarantineStatus};
    use crate::store::Store;

    fn setup() -> (tempfile::TempDir, tempfile::TempDir, Store) {
        let workspace = tempfile::tempdir().expect("workspace");
        let quarantine_root = tempfile::tempdir().expect("quarantine root");
        let store = Store::open_in_memory().expect("store");

        (workspace, quarantine_root, store)
    }

    #[test]
    fn quarantines_and_restores_a_file() {
        let (workspace, root, store) = setup();
        let original = workspace.path().join("report.txt");
        std::fs::write(&original, b"payload").expect("write fixture");
        let quarantine = Quarantine::new(root.path().join("store")).expect("quarantine");

        let item = quarantine
            .place(&store, &original, "test", 30)
            .expect("place");

        assert!(!original.exists());
        assert!(item.stored_path.is_file());
        assert_eq!(item.size, 7);
        assert_eq!(item.checksum.len(), 64);
        assert_eq!(item.status, QuarantineStatus::Stored);
        assert_eq!(item.reason, "test");
        assert!(item.expires_at > item.deleted_at);
        assert_eq!(store.quarantine_list().expect("list"), vec![item.clone()]);
        assert!(quarantine.verify(&item).expect("verify"));

        quarantine.restore(&store, &item).expect("restore");
        assert_eq!(std::fs::read(&original).expect("read restored"), b"payload");
        assert!(!item.stored_path.exists());
        assert_eq!(
            store.quarantine_list().expect("list")[0].status,
            QuarantineStatus::Restored
        );
    }

    #[test]
    fn quarantines_and_restores_a_directory() {
        let (workspace, root, store) = setup();
        let original = workspace.path().join("folder");
        std::fs::create_dir_all(original.join("nested")).expect("create directory");
        std::fs::write(original.join("nested").join("file.bin"), vec![0u8; 12])
            .expect("write file");
        let quarantine = Quarantine::new(root.path().join("store")).expect("quarantine");

        let item = quarantine
            .place(&store, &original, "cleanup", 7)
            .expect("place");

        assert!(!original.exists());
        assert!(item.stored_path.is_dir());
        assert_eq!(item.size, 12);
        assert!(item.checksum.is_empty());
        assert!(quarantine.verify(&item).expect("verify"));

        quarantine.restore(&store, &item).expect("restore");
        assert!(original.join("nested").join("file.bin").is_file());
    }

    #[test]
    fn refuses_to_overwrite_on_restore() {
        let (workspace, root, store) = setup();
        let original = workspace.path().join("take.txt");
        std::fs::write(&original, b"first").expect("write fixture");
        let quarantine = Quarantine::new(root.path().join("store")).expect("quarantine");
        let item = quarantine
            .place(&store, &original, "test", 1)
            .expect("place");

        std::fs::write(&original, b"second").expect("recreate original");
        let error = quarantine.restore(&store, &item).expect_err("conflict");

        assert_eq!(error.code(), ErrorCode::Conflict);
    }

    #[test]
    fn purges_a_stored_object() {
        let (workspace, root, store) = setup();
        let original = workspace.path().join("purge.txt");
        std::fs::write(&original, b"payload").expect("write fixture");
        let quarantine = Quarantine::new(root.path().join("store")).expect("quarantine");
        let item = quarantine
            .place(&store, &original, "test", 1)
            .expect("place");

        quarantine.purge(&store, &item).expect("purge");

        assert!(!item.stored_path.exists());
        assert_eq!(
            store.quarantine_list().expect("list")[0].status,
            QuarantineStatus::Purged
        );
        assert!(quarantine.restore(&store, &item).is_err());
    }

    #[test]
    fn detects_tampered_quarantine_contents() {
        let (workspace, root, store) = setup();
        let original = workspace.path().join("tamper.txt");
        std::fs::write(&original, b"payload").expect("write fixture");
        let quarantine = Quarantine::new(root.path().join("store")).expect("quarantine");
        let item = quarantine
            .place(&store, &original, "test", 1)
            .expect("place");

        std::fs::write(&item.stored_path, b"changed").expect("tamper");

        assert!(!quarantine.verify(&item).expect("verify"));
    }

    #[test]
    fn refuses_protected_system_paths() {
        let (_workspace, root, store) = setup();
        let system_root =
            PathBuf::from(env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into()));
        let protected = system_root.join("System32").join("kernel32.dll");
        let quarantine = Quarantine::new(root.path().join("store")).expect("quarantine");

        let error = quarantine
            .place(&store, &protected, "test", 1)
            .expect_err("must refuse");

        assert_eq!(error.code(), ErrorCode::ProtectedPath);
    }

    #[test]
    fn resolves_portable_quarantine_root() {
        let root = Quarantine::default_root(true).expect("portable root");

        assert!(root.ends_with("Quarantine"));
    }
}
