//! POSIX paths: length limits, case rules and mount-aware volume roots.
//!
//! Linux file names are case-sensitive, so [`equivalent_key`] keeps the exact
//! spelling instead of folding case. The limit is the kernel's `PATH_MAX`
//! (4096 bytes including the terminating NUL; the check uses 4096 bytes for the
//! path content). Mount points come from `/proc/self/mounts`, whose fields are
//! escaped as octal sequences such as `\040` for a space.

use std::{
    ffi::OsString,
    os::unix::ffi::OsStringExt as _,
    path::{Path, PathBuf},
};

use freespace_core::{Error, Result};

/// Maximum path length in bytes, matching the Linux `PATH_MAX`.
pub const PATH_MAX: usize = 4096;

/// One record of `/proc/self/mounts`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MountEntry {
    /// Device or pseudo-device that is mounted.
    pub source: String,
    /// Directory the device is mounted on.
    pub mount_point: PathBuf,
    /// File-system type as reported by the kernel.
    pub file_system: String,
}

/// Resolves a path to an absolute form and validates its length.
///
/// The path is not canonicalised: symbolic links stay untouched so that scope
/// checks still see the path the user selected.
///
/// # Errors
/// Returns [`Error::InvalidPath`] when the path cannot be made absolute or is
/// longer than [`PATH_MAX`].
pub fn normalize(path: &Path) -> Result<PathBuf> {
    let absolute = std::path::absolute(path).map_err(|error| Error::InvalidPath {
        path: path.to_path_buf(),
        reason: error.to_string(),
    })?;
    validate_length(&absolute)?;

    Ok(absolute)
}

/// Renders a path for logs and reports.
#[must_use]
pub fn display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Rejects a path that exceeds [`PATH_MAX`] before any change.
///
/// # Errors
/// Returns [`Error::InvalidPath`] when the path is too long.
pub fn validate_length(path: &Path) -> Result<()> {
    let length = path.as_os_str().len();
    if length > PATH_MAX {
        return Err(Error::InvalidPath {
            path: path.to_path_buf(),
            reason: format!("path has {length} bytes, limit is {PATH_MAX}"),
        });
    }

    Ok(())
}

/// Comparison key that follows the Linux case rules.
///
/// Only the absolute form is normalised; the name itself keeps its case so that
/// `Report.txt` and `report.txt` stay different files.
#[must_use]
pub fn equivalent_key(path: &Path) -> String {
    normalize(path)
        .map(|normalized| display(&normalized))
        .unwrap_or_else(|_| display(path))
}

/// Returns the nearest mount point that contains `path`.
///
/// # Errors
/// Returns [`Error::InvalidPath`] when `path` cannot be made absolute.
pub fn volume_root(path: &Path) -> Result<PathBuf> {
    let normalized = normalize(path)?;

    Ok(mount_entry_for(&read_mounts(), &normalized)
        .map(|entry| entry.mount_point.clone())
        .unwrap_or_else(|| PathBuf::from("/")))
}

/// Joins a file name and validates the resulting length before any change.
///
/// # Errors
/// Returns [`Error::InvalidArgument`] for an invalid name and
/// [`Error::InvalidPath`] when the result is too long.
pub fn join_within_limit(parent: &Path, name: &str) -> Result<PathBuf> {
    if name.is_empty() || name == "." || name == ".." {
        return Err(Error::InvalidArgument(
            "a file name cannot be empty, `.` or `..`".to_owned(),
        ));
    }
    if name.contains('/') || name.contains('\0') {
        return Err(Error::InvalidArgument(format!(
            "`{name}` is not a valid file name"
        )));
    }

    let candidate = parent.join(name);
    validate_length(&candidate)?;

    Ok(candidate)
}

/// Verifies that `target` lies inside `root` (or is the root itself).
///
/// # Errors
/// Returns [`Error::Operation`] when the target escapes the selected scope.
pub fn ensure_within(root: &Path, target: &Path) -> Result<()> {
    let root_path = normalize(root)?;
    let target_path = normalize(target)?;

    if target_path == root_path || target_path.starts_with(&root_path) {
        return Ok(());
    }

    Err(Error::Operation(format!(
        "`{}` is outside the selected scope `{}`",
        display(&target_path),
        display(&root_path)
    )))
}

/// Returns the longest mount point from `entries` that contains `path`.
pub(crate) fn mount_entry_for<'a>(
    entries: &'a [MountEntry],
    path: &Path,
) -> Option<&'a MountEntry> {
    entries
        .iter()
        .filter(|entry| path.starts_with(&entry.mount_point))
        .max_by_key(|entry| entry.mount_point.as_os_str().len())
}

/// Reads the kernel mount table.
///
/// `/proc/self/mounts` is the authoritative source; `/etc/mtab` is only a
/// fallback for systems without a mounted procfs. An unreadable table yields an
/// empty list so that callers fall back to `/` instead of failing.
pub(crate) fn read_mounts() -> Vec<MountEntry> {
    for table in ["/proc/self/mounts", "/etc/mtab"] {
        if let Ok(text) = std::fs::read_to_string(table) {
            return parse_mounts(&text);
        }
    }

    Vec::new()
}

/// Parses the contents of `/proc/self/mounts`.
pub(crate) fn parse_mounts(text: &str) -> Vec<MountEntry> {
    text.lines()
        .filter_map(parse_mount_line)
        .collect::<Vec<_>>()
}

fn parse_mount_line(line: &str) -> Option<MountEntry> {
    let mut fields = line.split_whitespace();
    let source = fields.next()?;
    let mount_point = fields.next()?;
    let file_system = fields.next()?;

    Some(MountEntry {
        source: String::from_utf8_lossy(&decode_mount_bytes(source)).into_owned(),
        mount_point: PathBuf::from(OsString::from_vec(decode_mount_bytes(mount_point))),
        file_system: file_system.to_owned(),
    })
}

/// Decodes the octal escapes used by `/proc/self/mounts` into raw bytes.
pub(crate) fn decode_mount_bytes(field: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(field.len());
    let raw = field.as_bytes();
    let mut index = 0;

    while index < raw.len() {
        if raw[index] == b'\\' && index + 3 < raw.len() {
            let digits = &raw[index + 1..index + 4];
            if let Ok(text) = std::str::from_utf8(digits) {
                if let Ok(value) = u8::from_str_radix(text, 8) {
                    bytes.push(value);
                    index += 4;
                    continue;
                }
            }
        }
        bytes.push(raw[index]);
        index += 1;
    }

    bytes
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{PATH_MAX, ensure_within, equivalent_key, parse_mounts, validate_length};

    const MOUNTS: &str = "\
/dev/sda1 / ext4 rw,relatime 0 0
proc /proc proc rw,nosuid,nodev,noexec 0 0
tmpfs /tmp\\040space tmpfs rw,nosuid,nodev 0 0
/dev/sdb1 /mnt/My\\040Disk vfat rw,relatime 0 0
server:/export /mnt/net\\011work nfs4 rw 0 0
";

    #[test]
    fn parses_mount_fields_and_octal_escapes() {
        let mounts = parse_mounts(MOUNTS);

        assert_eq!(mounts.len(), 5);
        assert_eq!(mounts[0].mount_point, PathBuf::from("/"));
        assert_eq!(mounts[0].file_system, "ext4");
        assert_eq!(mounts[2].mount_point, PathBuf::from("/tmp space"));
        assert_eq!(mounts[3].mount_point, PathBuf::from("/mnt/My Disk"));
        assert_eq!(mounts[4].mount_point, PathBuf::from("/mnt/net\twork"));
        assert_eq!(mounts[4].source, "server:/export");

        assert!(
            super::mount_entry_for(&mounts, Path::new("/mnt/My Disk/a.txt"))
                .expect("longest mount")
                .mount_point
                .eq(Path::new("/mnt/My Disk"))
        );
        assert_eq!(
            super::mount_entry_for(&mounts, Path::new("/home/user"))
                .expect("root mount")
                .mount_point,
            PathBuf::from("/")
        );
    }

    #[test]
    fn rejects_paths_above_the_limit_and_accepts_the_boundary() {
        let boundary = PathBuf::from("/").join("a".repeat(PATH_MAX - 1));
        assert_eq!(boundary.as_os_str().len(), PATH_MAX);
        validate_length(&boundary).expect("the boundary is allowed");

        let too_long = PathBuf::from("/").join("a".repeat(PATH_MAX));
        let error = validate_length(&too_long).expect_err("one byte over the limit");
        assert_eq!(error.code(), freespace_core::ErrorCode::InvalidPath);
    }

    #[test]
    fn comparison_keys_keep_case() {
        assert_ne!(
            equivalent_key(Path::new("/tmp/Report.txt")),
            equivalent_key(Path::new("/tmp/report.txt"))
        );
    }

    #[test]
    fn scope_checks_respect_component_boundaries() {
        ensure_within(
            Path::new("/tmp/scope"),
            Path::new("/tmp/scope/inner/file.txt"),
        )
        .expect("inside");
        ensure_within(Path::new("/tmp/scope"), Path::new("/tmp/scope")).expect("the root itself");

        assert!(
            ensure_within(Path::new("/tmp/scope"), Path::new("/tmp/scope-other")).is_err(),
            "a shared name prefix is not a child"
        );
    }
}
