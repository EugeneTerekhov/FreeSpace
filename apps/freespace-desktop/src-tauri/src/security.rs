//! Command allowlist and path-scope protection.
//!
//! React never touches the file system: every path crosses a command, is
//! canonicalised and must stay inside the selected scope. The allowlist doubles
//! as the `app_capabilities` payload used by the parity matrix.

use std::path::{Path, PathBuf};

use freespace_app::{
    dto::AppCapabilities,
    error::{UiError, UiErrorCode},
};

use crate::events::EVENT_NAMES;

/// Commands implemented today, in contract order.
pub const COMMANDS: [&str; 79] = [
    "about_info",
    "about_licenses",
    "app_about_info",
    "app_bootstrap",
    "app_capabilities",
    "app_elevation_request",
    "app_elevation_status",
    "app_navigation_request",
    "app_settings_get",
    "app_settings_set",
    "app_window_state_get",
    "app_window_state_set",
    "archives_entries_page",
    "archives_export_list",
    "cleanup_candidates",
    "cleanup_empty_dirs_page",
    "duplicates_cancel",
    "duplicates_group",
    "duplicates_groups_page",
    "duplicates_start",
    "duplicates_status",
    "export_cancel",
    "export_preview",
    "export_start",
    "export_status",
    "journal_entry",
    "journal_page",
    "journal_undo",
    "long_paths_page",
    "long_paths_validate",
    "metadata_get",
    "monitor_events_page",
    "monitor_reveal_executable",
    "monitor_reveal_file",
    "monitor_set_exclusions",
    "monitor_start",
    "monitor_status",
    "monitor_stop",
    "operation_cancel",
    "operation_confirm",
    "operation_preview",
    "operation_status",
    "operation_undo",
    "owners_files_page",
    "owners_page",
    "rename_apply",
    "rename_preview",
    "reveal_in_explorer",
    "scan_cancel",
    "scan_sessions",
    "scan_start",
    "scan_status",
    "settings_get",
    "settings_reset",
    "settings_set",
    "settings_validate_paths",
    "size_details",
    "snapshot_volumes",
    "snapshots_compare",
    "snapshots_create",
    "snapshots_delete",
    "snapshots_diff_page",
    "snapshots_list",
    "snapshots_status",
    "top_files_page",
    "tree_children",
    "tree_node",
    "tree_path",
    "tree_root",
    "tree_search",
    "types_files_page",
    "types_summary",
    "unlock_locks",
    "unlock_release",
    "unlock_schedule_reboot",
    "volumes_current",
    "volumes_list",
    "volumes_refresh",
    "volumes_summary",
];

/// Version of the CSP policy shipped with this build.
pub const CSP_VERSION: u32 = 1;

/// Returns the capability payload for `app_capabilities`.
#[must_use]
pub fn capabilities() -> AppCapabilities {
    AppCapabilities {
        commands: COMMANDS
            .iter()
            .map(|command| (*command).to_owned())
            .collect(),
        events: EVENT_NAMES
            .iter()
            .map(|event| (*event).to_owned())
            .collect(),
        csp_version: CSP_VERSION,
        platform_capabilities: freespace_platform::Capabilities::current(),
    }
}

/// Returns `true` when `name` is an implemented command.
#[must_use]
pub fn is_allowed_command(name: &str) -> bool {
    COMMANDS.contains(&name)
}

/// Canonicalises a path and maps failures onto interface errors.
///
/// # Errors
/// Returns `E_NOT_FOUND` for missing objects, `E_ACCESS_DENIED` for denied
/// access and `E_IO` for anything else.
pub fn canonicalize(path: &Path) -> Result<PathBuf, UiError> {
    std::fs::canonicalize(path).map_err(|error| {
        let code = match error.kind() {
            std::io::ErrorKind::NotFound => UiErrorCode::NotFound,
            std::io::ErrorKind::PermissionDenied => UiErrorCode::AccessDenied,
            _ => UiErrorCode::Io,
        };
        UiError::new(code).with_details(error.to_string())
    })
}

/// Returns `true` when `target` is `root` or lies below it.
///
/// Comparison is case-insensitive on Windows and separator-normalised.
#[must_use]
pub fn is_within(root: &Path, target: &Path) -> bool {
    let root = normalize_text(root);
    let target = normalize_text(target);
    // A volume root keeps its trailing separator (`e:\`), so comparing raw
    // prefixes would reject every direct child (`e:\file.txt`). Compare without
    // the separator and require a boundary of its own instead.
    let root = root.trim_end_matches('\\');
    let target = target.trim_end_matches('\\');

    target == root || target.starts_with(&format!("{root}\\"))
}

/// A canonical directory that bounds every path of a task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    root: PathBuf,
}

impl Scope {
    /// Creates a scope from an existing directory.
    ///
    /// # Errors
    /// Returns the mapped canonicalisation error.
    pub fn new(root: &Path) -> Result<Self, UiError> {
        let root = canonicalize(root)?;
        if !root.is_dir() {
            return Err(UiError::validation("the scope root must be a directory"));
        }

        Ok(Self { root })
    }

    /// Canonical root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Canonicalises `path` and refuses to leave the scope.
    ///
    /// # Errors
    /// Returns `E_ACCESS_DENIED` when the canonical path is outside the scope
    /// and the canonicalisation error otherwise.
    pub fn ensure_within(&self, path: &Path) -> Result<PathBuf, UiError> {
        let canonical = canonicalize(path)?;
        if !is_within(&self.root, &canonical) {
            return Err(UiError::new(UiErrorCode::AccessDenied)
                .with_details("the path is outside the selected scope"));
        }

        Ok(canonical)
    }

    /// Returns `true` when `path` resolves inside the scope.
    #[must_use]
    pub fn contains(&self, path: &Path) -> bool {
        canonicalize(path).is_ok_and(|canonical| is_within(&self.root, &canonical))
    }
}

fn normalize_text(path: &Path) -> String {
    let mut text = path
        .to_string_lossy()
        .replace('/', "\\")
        .to_ascii_lowercase();
    while text.len() > 3 && text.ends_with('\\') {
        text.pop();
    }

    text
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{COMMANDS, Scope, capabilities, is_allowed_command, is_within};

    #[test]
    fn allowlist_is_sorted_and_unique() {
        let mut sorted = COMMANDS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), COMMANDS.len());
        assert_eq!(sorted.as_slice(), COMMANDS.as_slice());

        assert!(is_allowed_command("app_bootstrap"));
        assert!(!is_allowed_command("not_a_command"));
    }

    #[test]
    fn capabilities_match_the_allowlist() {
        let capabilities = capabilities();
        assert_eq!(capabilities.commands.len(), COMMANDS.len());
        assert!(capabilities.events.contains(&"scan-progress".to_owned()));
        assert_eq!(capabilities.csp_version, super::CSP_VERSION);
    }

    #[test]
    fn capabilities_describe_the_running_platform() {
        let capabilities = capabilities();
        let expected = freespace_platform::Capabilities::current();

        assert_eq!(capabilities.platform_capabilities, expected);
        assert_eq!(
            capabilities.platform_capabilities.platform,
            expected.platform
        );
        // The GUI must not claim a capability the CLI would reject.
        assert!(capabilities.platform_capabilities.permanent);
    }

    #[test]
    fn keeps_paths_inside_the_scope() {
        let directory = tempfile::tempdir().expect("temp dir");
        let nested = directory.path().join("nested");
        fs::create_dir(&nested).expect("dir");
        let file = nested.join("payload.bin");
        fs::write(&file, b"x").expect("file");

        let scope = Scope::new(directory.path()).expect("scope");
        assert!(scope.ensure_within(&file).is_ok());
        assert!(scope.ensure_within(&nested).is_ok());
        assert!(scope.contains(&file));

        let outside = tempfile::tempdir().expect("outside");
        assert!(scope.ensure_within(outside.path()).is_err());
        assert!(!scope.contains(outside.path()));
    }

    #[test]
    fn refuses_parent_traversal() {
        let directory = tempfile::tempdir().expect("temp dir");
        let nested = directory.path().join("nested");
        fs::create_dir(&nested).expect("dir");

        let scope = Scope::new(&nested).expect("scope");
        let escape = nested.join("..");
        assert!(scope.ensure_within(&escape).is_err());
    }

    #[test]
    fn accepts_direct_children_of_a_volume_root() {
        for root in [r"E:\", r"E:", r"e:\"] {
            assert!(is_within(
                std::path::Path::new(root),
                std::path::Path::new(r"E:\222222222222222222.txt")
            ));
            assert!(is_within(
                std::path::Path::new(root),
                std::path::Path::new(r"e:\folder\payload.bin")
            ));
            assert!(is_within(
                std::path::Path::new(root),
                std::path::Path::new(root)
            ));
            assert!(!is_within(
                std::path::Path::new(root),
                std::path::Path::new(r"F:\payload.bin")
            ));
        }

        // A directory root still needs a separator boundary.
        assert!(is_within(
            std::path::Path::new(r"E:\folder"),
            std::path::Path::new(r"E:\folder\payload.bin")
        ));
        assert!(!is_within(
            std::path::Path::new(r"E:\folder"),
            std::path::Path::new(r"E:\folder-other\payload.bin")
        ));
    }

    #[test]
    fn compares_case_insensitively() {
        assert!(is_within(
            std::path::Path::new(r"C:\Users\Public"),
            std::path::Path::new(r"c:\users\public\file.txt")
        ));
        assert!(!is_within(
            std::path::Path::new(r"C:\Users\Public"),
            std::path::Path::new(r"C:\Users\Publicity\file.txt")
        ));
    }
}
