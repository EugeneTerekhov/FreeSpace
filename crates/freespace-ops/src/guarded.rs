//! Scope and system-path guards shared by every destructive operation
//! (FR-045, FR-046).
//!
//! The guard is a pure preparation step: it never touches the file system, it
//! only classifies targets so the caller can raise elevation or ask for the
//! extra confirmation before anything is changed.

use std::path::{Path, PathBuf};

use freespace_core::{Error, Result};
use freespace_platform::paths;

use crate::confirm;

/// Result of inspecting a target list against the selected scope.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct GuardReport {
    /// Targets that lie outside the selected scope.
    pub outside_scope: Vec<PathBuf>,
    /// Protected system paths.
    pub protected: Vec<PathBuf>,
    /// Whether elevation is required.
    pub requires_elevation: bool,
    /// Whether the caller must ask for the extra protected-path confirmation.
    pub requires_extra_confirmation: bool,
}

impl GuardReport {
    /// Returns `true` when the operation can run without extra steps.
    #[must_use]
    pub fn is_clear(&self) -> bool {
        self.outside_scope.is_empty() && self.protected.is_empty()
    }
}

/// Classifies `targets` against `scope`.
#[must_use]
pub fn inspect(scope: &Path, targets: &[PathBuf]) -> GuardReport {
    let mut report = GuardReport::default();

    for target in targets {
        if paths::ensure_within(scope, target).is_err() {
            report.outside_scope.push(target.clone());
        }
        if confirm::is_protected_path(target) {
            report.protected.push(target.clone());
        }
    }

    report.requires_elevation = !report.protected.is_empty();
    report.requires_extra_confirmation = report.requires_elevation;

    report
}

/// Enforces the scope and protected-path rules.
///
/// # Errors
/// Returns [`Error::Operation`] for a target outside the scope and
/// [`Error::ProtectedPath`] when `extra_confirmed` is `false`.
pub fn enforce(scope: &Path, targets: &[PathBuf], extra_confirmed: bool) -> Result<()> {
    let report = inspect(scope, targets);

    if let Some(path) = report.outside_scope.first() {
        return Err(Error::Operation(format!(
            "`{}` is outside the selected scope `{}`",
            paths::display(path),
            paths::display(scope)
        )));
    }
    if !report.protected.is_empty() && !extra_confirmed {
        return Err(Error::ProtectedPath(report.protected[0].clone()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use freespace_core::ErrorCode;

    use super::{enforce, inspect};

    #[test]
    fn classifies_outside_and_protected_targets() {
        let directory = tempfile::tempdir().expect("temp dir");
        let inside = directory.path().join("inside.txt");
        std::fs::write(&inside, b"x").expect("write");
        let outside = directory.path().join("..").join("outside.txt");
        let system_root =
            PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into()));
        let protected = system_root.join("System32").join("kernel32.dll");

        let report = inspect(
            directory.path(),
            &[inside, outside.clone(), protected.clone()],
        );

        assert!(report.outside_scope.contains(&outside));
        assert_eq!(report.protected.len(), 1);
        assert_eq!(report.protected[0], protected);
        assert!(report.requires_elevation);
        assert!(report.requires_extra_confirmation);
        assert!(!report.is_clear());
    }

    #[test]
    fn enforces_scope_and_extra_confirmation() {
        let directory = tempfile::tempdir().expect("temp dir");
        let inside = directory.path().join("inside.txt");
        std::fs::write(&inside, b"x").expect("write");

        enforce(directory.path(), std::slice::from_ref(&inside), false).expect("clear");
        let outside = directory.path().join("..").join("outside.txt");
        let error = enforce(directory.path(), &[outside], false).expect_err("outside");
        assert_eq!(error.code(), ErrorCode::Operation);

        let system_root =
            PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into()));
        let protected = system_root.join("System32").join("kernel32.dll");
        let error = enforce(&system_root, &[protected], false).expect_err("protected");
        assert_eq!(error.code(), ErrorCode::ProtectedPath);
    }
}
