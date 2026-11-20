//! Linux protected paths.
//!
//! A destructive operation over these directories needs an additional explicit
//! confirmation. As on Windows the list is deliberately small and meaningful:
//! the root itself plus the system directories, but never `/home`, `/tmp`,
//! `/opt` or `/srv`, which belong to the user and to installed applications.
//! Only the listed directory and its children are protected; `/` is treated as
//! the root entry itself rather than as a prefix of every absolute path.

use std::path::Path;

/// System directories that must never be deleted casually.
pub const PROTECTED_PATHS: &[&str] = &[
    "/", "/bin", "/boot", "/dev", "/etc", "/lib", "/lib64", "/proc", "/root", "/sbin", "/sys",
    "/usr", "/var",
];

/// Returns `true` when `path` is a protected system path or lies inside one.
///
/// Relative paths are resolved against the current directory first, so a
/// relative spelling cannot bypass the check.
#[must_use]
pub fn is_protected_path(path: &Path) -> bool {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    if absolute == Path::new("/") {
        return true;
    }

    PROTECTED_PATHS
        .iter()
        .filter(|root| **root != "/")
        .any(|root| {
            let root = Path::new(root);
            absolute == root || absolute.starts_with(root)
        })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::is_protected_path;

    #[test]
    fn protects_the_root_and_system_directories() {
        assert!(is_protected_path(Path::new("/")));
        assert!(is_protected_path(Path::new("/etc")));
        assert!(is_protected_path(Path::new("/etc/passwd")));
        assert!(is_protected_path(Path::new("/usr/local/bin/tool")));
        assert!(is_protected_path(Path::new("/var/lib/freespace")));
    }

    #[test]
    fn leaves_user_and_temporary_directories_alone() {
        assert!(!is_protected_path(Path::new("/home/alice")));
        assert!(!is_protected_path(Path::new("/tmp/work")));
        assert!(!is_protected_path(Path::new("/opt/app")));
        assert!(!is_protected_path(Path::new("/srv/data")));
        assert!(!is_protected_path(Path::new("/mnt/disk")));
    }

    #[test]
    fn checks_whole_path_components() {
        assert!(!is_protected_path(Path::new("/etcetera")));
        assert!(!is_protected_path(Path::new("/various")));
        assert!(!is_protected_path(Path::new("/usr-local")));
    }
}
