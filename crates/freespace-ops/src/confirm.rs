//! Preview and confirmation engine for destructive operations.
//!
//! Every mutating operation must show the affected objects, their total size
//! and the consequences of each deletion mode before anything happens
//! (FR-042, FR-043). This module builds that preview and enforces the
//! confirmation, scope and free-space guards.

#[cfg(windows)]
use std::env;
use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use freespace_core::{Error, Result};
use freespace_platform::paths;

use crate::operation::{DeleteMode, Operation, OperationKind};
use crate::path_compat;

/// Kind of an object shown in a preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PreviewItemKind {
    /// Regular file.
    File,
    /// Directory.
    Directory,
    /// Symbolic link or junction.
    Link,
    /// The path does not exist any more.
    Missing,
    /// Metadata could not be read.
    Unknown,
}

/// One object shown in a preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewItem {
    /// Full path.
    pub path: PathBuf,
    /// Size in bytes.
    pub size: u64,
    /// Kind of object.
    pub kind: PreviewItemKind,
}

/// Information about one selectable deletion mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteOptionInfo {
    /// Mode.
    pub mode: DeleteMode,
    /// Whether the mode can be used for this preview.
    pub available: bool,
}

impl DeleteOptionInfo {
    /// Translation key describing the consequences of the mode.
    #[must_use]
    pub const fn consequence_key(self) -> &'static str {
        delete_mode_consequence(self.mode)
    }
}

/// Translation key describing the consequences of a deletion mode.
#[must_use]
pub const fn delete_mode_consequence(mode: DeleteMode) -> &'static str {
    match mode {
        DeleteMode::RecycleBin => "delete.mode.recycle_bin",
        DeleteMode::Permanent => "delete.mode.permanent",
    }
}

/// Free-space shortage detected while building a preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpaceShortage {
    /// Bytes required.
    pub required: u64,
    /// Bytes available.
    pub available: u64,
}

/// Everything the user must see before confirming an operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preview {
    /// Operation kind.
    pub kind: OperationKind,
    /// Affected objects.
    pub items: Vec<PreviewItem>,
    /// Total size of the affected objects.
    pub total_bytes: u64,
    /// Number of affected objects.
    pub total_count: u32,
    /// Selectable deletion modes.
    pub delete_options: Vec<DeleteOptionInfo>,
    /// Whether elevated rights are required.
    pub requires_elevation: bool,
    /// Whether an additional explicit confirmation is required.
    pub requires_extra_confirmation: bool,
    /// Free-space shortage, when the operation cannot run as planned.
    pub insufficient_space: Option<SpaceShortage>,
    /// Human-readable warnings (missing objects, protected paths, ...).
    pub warnings: Vec<String>,
}

impl Preview {
    /// Verifies that the selected deletion mode has enough free space.
    ///
    /// # Errors
    /// Returns [`Error::InsufficientSpace`] when a copying mode cannot run.
    pub fn ensure_space_for(&self, mode: DeleteMode) -> Result<()> {
        if mode == DeleteMode::Permanent {
            return Ok(());
        }

        match self.insufficient_space {
            Some(shortage) => Err(Error::InsufficientSpace {
                required: shortage.required,
                available: shortage.available,
            }),
            None => Ok(()),
        }
    }
}

/// Builds a preview for deleting `paths`.
///
/// `available_bytes` is the free space on the target volume; when it is known,
/// the preview reports whether the copying deletion modes can run.
///
/// # Errors
/// Returns [`Error::InvalidArgument`] for an empty target list and
/// [`Error::InvalidPath`] when a path cannot be converted to verbatim form.
pub fn preview_delete(paths: &[PathBuf], available_bytes: Option<u64>) -> Result<Preview> {
    if paths.is_empty() {
        return Err(Error::InvalidArgument(
            "a delete preview needs at least one path".to_owned(),
        ));
    }

    let mut items = Vec::with_capacity(paths.len());
    let mut warnings = Vec::new();
    let mut total_bytes = 0u64;

    for path in paths {
        let verbatim = path_compat::to_verbatim(path)?;
        let (kind, size) = match fs::symlink_metadata(&verbatim) {
            Ok(metadata) => {
                let file_type = metadata.file_type();
                let kind = if file_type.is_symlink() {
                    PreviewItemKind::Link
                } else if file_type.is_dir() {
                    PreviewItemKind::Directory
                } else {
                    PreviewItemKind::File
                };
                let size = if file_type.is_dir() {
                    let (size, directory_warnings) = directory_size(path);
                    warnings.extend(directory_warnings);
                    size
                } else {
                    metadata.len()
                };

                (kind, size)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                warnings.push(format!("missing: {}", paths::display(path)));
                (PreviewItemKind::Missing, 0)
            }
            Err(error) => {
                warnings.push(format!("cannot read `{}`: {error}", paths::display(path)));
                (PreviewItemKind::Unknown, 0)
            }
        };

        total_bytes = total_bytes.saturating_add(size);
        items.push(PreviewItem {
            path: path.clone(),
            size,
            kind,
        });
    }

    let protected: Vec<&Path> = paths
        .iter()
        .map(PathBuf::as_path)
        .filter(|path| is_protected_path(path))
        .collect();
    for path in &protected {
        warnings.push(format!("protected path: {}", paths::display(path)));
    }

    let requires_elevation = !protected.is_empty();
    let insufficient_space = available_bytes
        .filter(|available| *available < total_bytes)
        .map(|available| SpaceShortage {
            required: total_bytes,
            available,
        });

    Ok(Preview {
        kind: OperationKind::Delete,
        items,
        total_bytes,
        total_count: u32::try_from(paths.len()).unwrap_or(u32::MAX),
        delete_options: delete_options(),
        requires_elevation,
        requires_extra_confirmation: requires_elevation,
        insufficient_space,
        warnings,
    })
}

/// Builds a preview for an already planned operation.
#[must_use]
pub fn preview_operation(operation: &Operation, available_bytes: Option<u64>) -> Preview {
    let items: Vec<PreviewItem> = operation
        .targets
        .iter()
        .map(|target| PreviewItem {
            path: target.path.clone(),
            size: target.size,
            kind: if target.size == 0 {
                PreviewItemKind::Unknown
            } else {
                PreviewItemKind::File
            },
        })
        .collect();
    let total_bytes = operation.total_bytes();
    let protected: Vec<&Path> = operation
        .targets
        .iter()
        .map(|target| target.path.as_path())
        .filter(|path| is_protected_path(path))
        .collect();
    let requires_elevation = !protected.is_empty() || operation.requires_elevation;
    let insufficient_space = available_bytes
        .filter(|available| *available < total_bytes)
        .map(|available| SpaceShortage {
            required: total_bytes,
            available,
        });

    Preview {
        kind: operation.kind,
        items,
        total_bytes,
        total_count: u32::try_from(operation.targets.len()).unwrap_or(u32::MAX),
        delete_options: if operation.kind == OperationKind::Delete {
            delete_options()
        } else {
            Vec::new()
        },
        requires_elevation,
        requires_extra_confirmation: requires_elevation,
        insufficient_space,
        warnings: protected
            .iter()
            .map(|path| format!("protected path: {}", paths::display(path)))
            .collect(),
    }
}

/// Enforces the explicit confirmation flag.
///
/// # Errors
/// Returns [`Error::ConfirmationRequired`] when `confirmed` is `false`.
pub fn require_confirmation(confirmed: bool) -> Result<()> {
    if confirmed {
        Ok(())
    } else {
        Err(Error::ConfirmationRequired(
            "a destructive operation needs an explicit confirmation".to_owned(),
        ))
    }
}

/// Verifies that every target lies inside the selected scope.
///
/// # Errors
/// Returns [`Error::Operation`] for the first target outside `scope`.
pub fn validate_scope(scope: &Path, targets: &[PathBuf]) -> Result<()> {
    for target in targets {
        paths::ensure_within(scope, target)?;
    }

    Ok(())
}

/// Returns `true` when the path is a protected system location.
///
/// Protected locations always require elevation and an extra confirmation. The
/// check is purely lexical, so it refuses a protected path even when it has
/// already been removed.
#[must_use]
pub fn is_protected_path(path: &Path) -> bool {
    #[cfg(unix)]
    {
        freespace_platform::guarded::is_protected_path(path)
    }

    #[cfg(windows)]
    {
        let normalized = paths::normalize(path)
            .map(|path| path_compat::strip_verbatim(&path))
            .unwrap_or_else(|_| path.to_path_buf());

        if let Ok(root) = paths::volume_root(&normalized) {
            let root_text = paths::display(&root).to_lowercase();
            let path_text = paths::display(&normalized).to_lowercase();

            if path_text.trim_end_matches('\\') == root_text.trim_end_matches('\\') {
                return true;
            }
        }

        if normalized.components().any(|component| {
            let text = component.as_os_str().to_string_lossy().to_lowercase();

            text == "system volume information" || text == "$recycle.bin"
        }) {
            return true;
        }

        protected_roots()
            .iter()
            .any(|root| paths::ensure_within(root, &normalized).is_ok())
    }
}

#[cfg(windows)]
fn protected_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    for variable in [
        "SystemRoot",
        "windir",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramData",
    ] {
        if let Some(value) = env::var_os(variable) {
            roots.push(PathBuf::from(value));
        }
    }

    if roots.is_empty() {
        roots.push(PathBuf::from(r"C:\Windows"));
    }

    roots
}

fn delete_options() -> Vec<DeleteOptionInfo> {
    [DeleteMode::RecycleBin, DeleteMode::Permanent]
        .into_iter()
        .map(|mode| DeleteOptionInfo {
            mode,
            available: true,
        })
        .collect()
}

/// Sums the file lengths below a directory, collecting warnings for unreadable
/// entries instead of failing the whole preview.
fn directory_size(path: &Path) -> (u64, Vec<String>) {
    let mut total = 0u64;
    let mut warnings = Vec::new();
    let mut stack = vec![path.to_path_buf()];

    while let Some(directory) = stack.pop() {
        let verbatim = match path_compat::to_verbatim(&directory) {
            Ok(verbatim) => verbatim,
            Err(error) => {
                warnings.push(format!(
                    "cannot read `{}`: {error}",
                    paths::display(&directory)
                ));
                continue;
            }
        };

        let entries = match fs::read_dir(&verbatim) {
            Ok(entries) => entries,
            Err(error) => {
                warnings.push(format!(
                    "cannot read `{}`: {error}",
                    paths::display(&directory)
                ));
                continue;
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    warnings.push(format!(
                        "cannot read an entry of `{}`: {error}",
                        paths::display(&directory)
                    ));
                    continue;
                }
            };
            match entry.metadata() {
                Ok(metadata) if metadata.is_dir() => stack.push(entry.path()),
                Ok(metadata) => total = total.saturating_add(metadata.len()),
                Err(error) => warnings.push(format!(
                    "cannot read `{}`: {error}",
                    paths::display(&entry.path())
                )),
            }
        }
    }

    (total, warnings)
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use std::env;
    use std::path::PathBuf;

    use freespace_core::ErrorCode;

    use super::{
        PreviewItemKind, is_protected_path, preview_delete, preview_operation,
        require_confirmation, validate_scope,
    };
    use crate::operation::{DeleteMode, Operation, OperationKind, OperationTarget};

    fn fixture() -> (tempfile::TempDir, Vec<PathBuf>) {
        let directory = tempfile::tempdir().expect("temp dir");
        let first = directory.path().join("first.txt");
        let second = directory.path().join("second.txt");
        std::fs::write(&first, vec![b'a'; 10]).expect("write first");
        std::fs::write(&second, vec![b'b'; 20]).expect("write second");

        (directory, vec![first, second])
    }

    #[test]
    fn previews_files_and_total_size() {
        let (_directory, paths) = fixture();
        let preview = preview_delete(&paths, Some(1_000)).expect("preview");

        assert_eq!(preview.kind, OperationKind::Delete);
        assert_eq!(preview.total_count, 2);
        assert_eq!(preview.total_bytes, 30);
        assert_eq!(preview.delete_options.len(), 2);
        assert!(!preview.requires_elevation);
        assert!(preview.insufficient_space.is_none());
        assert!(preview.warnings.is_empty());
        assert!(
            preview
                .items
                .iter()
                .all(|item| item.kind == PreviewItemKind::File)
        );
        assert!(preview.ensure_space_for(DeleteMode::RecycleBin).is_ok());
    }

    #[test]
    fn previews_directories_recursively() {
        let directory = tempfile::tempdir().expect("temp dir");
        let folder = directory.path().join("folder");
        std::fs::create_dir_all(folder.join("nested")).expect("create folder");
        std::fs::write(folder.join("top.bin"), [0u8; 5]).expect("write file");
        std::fs::write(folder.join("nested").join("deep.bin"), [0u8; 12]).expect("write file");

        let preview = preview_delete(std::slice::from_ref(&folder), None).expect("preview");

        assert_eq!(preview.items[0].kind, PreviewItemKind::Directory);
        assert_eq!(preview.total_bytes, 17);
        assert!(preview.warnings.is_empty());
    }

    #[test]
    fn reports_missing_paths() {
        let directory = tempfile::tempdir().expect("temp dir");
        let missing = directory.path().join("gone.txt");
        let preview = preview_delete(&[missing], None).expect("preview");

        assert_eq!(preview.items[0].kind, PreviewItemKind::Missing);
        assert_eq!(preview.total_bytes, 0);
        assert!(preview.warnings[0].contains("missing"));
    }

    #[test]
    fn rejects_empty_previews() {
        assert!(preview_delete(&[], None).is_err());
    }

    #[test]
    fn detects_insufficient_space_for_copying_modes() {
        let (_directory, paths) = fixture();
        let preview = preview_delete(&paths, Some(10)).expect("preview");
        let shortage = preview.insufficient_space.expect("shortage");

        assert_eq!(shortage.required, 30);
        assert_eq!(shortage.available, 10);

        let error = preview
            .ensure_space_for(DeleteMode::RecycleBin)
            .expect_err("must refuse");
        assert_eq!(error.code(), ErrorCode::InsufficientSpace);
        assert!(
            preview.ensure_space_for(DeleteMode::Permanent).is_ok(),
            "permanent deletion does not need extra space"
        );
    }

    #[cfg(windows)]
    #[test]
    fn marks_system_paths_as_protected() {
        let system_root =
            PathBuf::from(env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into()));
        let system_path = system_root.join("System32").join("kernel32.dll");
        let directory = tempfile::tempdir().expect("temp dir");
        let preview = preview_delete(std::slice::from_ref(&system_path), None).expect("preview");

        assert!(is_protected_path(&system_path));
        assert!(preview.requires_elevation);
        assert!(preview.requires_extra_confirmation);
        assert!(!is_protected_path(directory.path()));
        assert!(is_protected_path(
            PathBuf::from(r"C:\System Volume Information").as_path()
        ));
    }

    #[test]
    fn enforces_the_selected_scope() {
        let (directory, paths) = fixture();
        validate_scope(directory.path(), &paths).expect("inside scope");

        let outside = directory
            .path()
            .parent()
            .expect("parent")
            .join("outside.txt");
        assert!(validate_scope(directory.path(), &[outside]).is_err());
    }

    #[test]
    fn requires_explicit_confirmation() {
        assert!(require_confirmation(true).is_ok());
        let error = require_confirmation(false).expect_err("must refuse");
        assert_eq!(error.code(), ErrorCode::ConfirmationRequired);
    }

    #[test]
    fn previews_a_planned_operation() {
        let operation = Operation::new(
            OperationKind::Delete,
            vec![
                OperationTarget::new(r"C:\temp\a.txt", 5),
                OperationTarget::new(r"C:\temp\b.txt", 7),
            ],
        )
        .with_delete_mode(DeleteMode::Permanent)
        .expect("mode");
        let preview = preview_operation(&operation, Some(100));

        assert_eq!(preview.total_bytes, 12);
        assert_eq!(preview.total_count, 2);
        assert_eq!(preview.delete_options.len(), 2);
        assert!(preview.insufficient_space.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn marks_linux_system_paths_as_protected() {
        let directory = tempfile::tempdir().expect("temp dir");

        assert!(is_protected_path(PathBuf::from("/etc/passwd").as_path()));
        assert!(is_protected_path(PathBuf::from("/usr/bin/env").as_path()));
        assert!(!is_protected_path(directory.path()));
        assert!(!is_protected_path(
            PathBuf::from("/tmp/freespace-test").as_path()
        ));
    }
}
