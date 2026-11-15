//! Verbatim path handling for long and Unicode paths.
//!
//! FreeSpace never depends on the system-wide `LongPathsEnabled` policy: every
//! file-system call receives a `\\?\` (or `\\?\UNC\`) path. This module owns the
//! conversion, length validation and scope checks; it works on UTF-16 units and
//! therefore keeps unpaired surrogate names intact.

use std::{
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
};

use freespace_core::{Error, Result};

/// Maximum path length accepted by the Windows object manager, in UTF-16 units.
///
/// The documented limit is 32 767 units; FreeSpace stays one unit below so that
/// a terminator always fits.
pub const MAX_PATH_CHARS: usize = 32_000;

const BACKSLASH: u16 = b'\\' as u16;
const QUESTION: u16 = b'?' as u16;
const VERBATIM_PREFIX: [u16; 4] = [BACKSLASH, BACKSLASH, QUESTION, BACKSLASH];
const UNC_PREFIX: [u16; 2] = [BACKSLASH, BACKSLASH];

/// Returns `true` when the path already uses a verbatim prefix.
#[must_use]
pub fn is_verbatim(path: &Path) -> bool {
    path.as_os_str()
        .encode_wide()
        .take(VERBATIM_PREFIX.len())
        .eq(VERBATIM_PREFIX)
}

/// Converts a path to its absolute verbatim form.
///
/// Relative paths are resolved against the current directory first. UNC paths
/// become `\\?\UNC\server\share\...`; drive paths become `\\?\C:\...`.
///
/// # Errors
/// Returns [`Error::InvalidPath`] when the resulting path exceeds
/// [`MAX_PATH_CHARS`] or the current directory cannot be resolved.
pub fn to_verbatim(path: &Path) -> Result<PathBuf> {
    validate_length(path)?;

    let absolute = std::path::absolute(path).map_err(|error| Error::InvalidPath {
        path: path.to_path_buf(),
        reason: error.to_string(),
    })?;

    if is_verbatim(&absolute) {
        return Ok(absolute);
    }

    let units: Vec<u16> = absolute.as_os_str().encode_wide().collect();
    let mut verbatim = Vec::with_capacity(units.len() + 8);

    if units.starts_with(&UNC_PREFIX) {
        verbatim.extend_from_slice(&VERBATIM_PREFIX);
        verbatim.extend("UNC\\".encode_utf16());
        verbatim.extend_from_slice(&units[UNC_PREFIX.len()..]);
    } else {
        verbatim.extend_from_slice(&VERBATIM_PREFIX);
        verbatim.extend_from_slice(&units);
    }

    let verbatim = PathBuf::from(std::ffi::OsString::from_wide(&verbatim));
    validate_length(&verbatim)?;

    Ok(verbatim)
}

/// Removes the verbatim prefix for display purposes.
///
/// The conversion works on UTF-16 units, so names containing unpaired
/// surrogates survive it unchanged.
#[must_use]
pub fn strip_verbatim(path: &Path) -> PathBuf {
    let units: Vec<u16> = path.as_os_str().encode_wide().collect();

    if units.starts_with(&VERBATIM_PREFIX) {
        let rest = &units[VERBATIM_PREFIX.len()..];
        if rest.starts_with(&[b'U' as u16, b'N' as u16, b'C' as u16, BACKSLASH]) {
            let mut display = UNC_PREFIX.to_vec();
            display.extend_from_slice(&rest[4..]);

            return PathBuf::from(std::ffi::OsString::from_wide(&display));
        }

        return PathBuf::from(std::ffi::OsString::from_wide(rest));
    }

    path.to_path_buf()
}

/// Renders a path for logs and reports, without the verbatim prefix.
#[must_use]
pub fn display(path: &Path) -> String {
    strip_verbatim(path).to_string_lossy().into_owned()
}

/// Validates that a path fits the Windows object-manager limit.
///
/// # Errors
/// Returns [`Error::InvalidPath`] when the path is longer than
/// [`MAX_PATH_CHARS`] UTF-16 units.
pub fn validate_length(path: &Path) -> Result<()> {
    let length = path.as_os_str().encode_wide().count();

    if length > MAX_PATH_CHARS {
        return Err(Error::InvalidPath {
            path: path.to_path_buf(),
            reason: format!("path has {length} characters, limit is {MAX_PATH_CHARS}"),
        });
    }

    Ok(())
}

/// Resolves a path to a verbatim absolute form without trailing separators.
///
/// Drive and UNC roots keep their trailing separator.
///
/// # Errors
/// Returns [`Error::InvalidPath`] when the path cannot be made absolute or is
/// too long.
pub fn normalize(path: &Path) -> Result<PathBuf> {
    let verbatim = to_verbatim(path)?;
    let mut units: Vec<u16> = verbatim.as_os_str().encode_wide().collect();
    let root_length = verbatim_root_length(&units);

    while units.len() > root_length && units.last() == Some(&BACKSLASH) {
        units.pop();
    }

    Ok(PathBuf::from(std::ffi::OsString::from_wide(&units)))
}

/// Returns the volume root of a path (`C:\` or `\\server\share\`).
///
/// # Errors
/// Returns [`Error::InvalidPath`] when the path has no root component.
pub fn volume_root(path: &Path) -> Result<PathBuf> {
    let absolute = std::path::absolute(path).map_err(|error| Error::InvalidPath {
        path: path.to_path_buf(),
        reason: error.to_string(),
    })?;
    let display_form = strip_verbatim(&absolute);
    let root = display_form
        .ancestors()
        .filter(|ancestor| !ancestor.as_os_str().is_empty())
        .last()
        .ok_or_else(|| Error::InvalidPath {
            path: path.to_path_buf(),
            reason: "path has no volume root".to_owned(),
        })?;

    Ok(root.to_path_buf())
}

/// Verifies that `target` lies inside `root` (or is the root itself).
///
/// Comparison is case-insensitive because Windows file names are.
///
/// # Errors
/// Returns [`Error::Operation`] when `target` escapes the selected scope.
pub fn ensure_within(root: &Path, target: &Path) -> Result<()> {
    let root_path = normalize(root)?;
    let target_path = normalize(target)?;
    let root_text = display(&root_path).to_lowercase();
    let target_text = display(&target_path).to_lowercase();

    let root_trimmed = root_text.trim_end_matches('\\');
    let target_trimmed = target_text.trim_end_matches('\\');
    let prefix = format!("{root_trimmed}\\");

    if target_trimmed == root_trimmed || target_trimmed.starts_with(&prefix) {
        return Ok(());
    }

    Err(Error::Operation(format!(
        "`{}` is outside the selected scope `{}`",
        display(&target_path),
        display(&root_path)
    )))
}

fn verbatim_root_length(units: &[u16]) -> usize {
    // `\\?\UNC\server\share\`
    let unc_prefix: Vec<u16> = "\\\\?\\UNC\\".encode_utf16().collect();

    if units.starts_with(&unc_prefix) {
        let rest = &units[unc_prefix.len()..];
        let mut separators = 0;
        for (index, unit) in rest.iter().enumerate() {
            if *unit == BACKSLASH {
                separators += 1;
                if separators == 2 {
                    return unc_prefix.len() + index + 1;
                }
            }
        }

        return units.len();
    }

    if units.starts_with(&VERBATIM_PREFIX) {
        // `\\?\C:\`
        return VERBATIM_PREFIX.len() + 3;
    }

    0
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{
        MAX_PATH_CHARS, display, ensure_within, is_verbatim, normalize, strip_verbatim,
        to_verbatim, validate_length, volume_root,
    };

    #[test]
    fn converts_drive_paths_to_verbatim() {
        let verbatim = to_verbatim(Path::new(r"C:\temp\file.txt")).expect("verbatim");

        assert!(is_verbatim(&verbatim));
        assert_eq!(verbatim, PathBuf::from(r"\\?\C:\temp\file.txt"));
        assert_eq!(
            strip_verbatim(&verbatim),
            PathBuf::from(r"C:\temp\file.txt")
        );
    }

    #[test]
    fn converts_unc_paths_to_verbatim() {
        let verbatim = to_verbatim(Path::new(r"\\server\share\folder")).expect("verbatim");

        assert_eq!(verbatim, PathBuf::from(r"\\?\UNC\server\share\folder"));
        assert_eq!(
            strip_verbatim(&verbatim),
            PathBuf::from(r"\\server\share\folder")
        );
    }

    #[test]
    fn keeps_already_verbatim_paths_unchanged() {
        let path = Path::new(r"\\?\C:\already\verbatim");

        assert_eq!(to_verbatim(path).expect("verbatim"), path);
    }

    #[test]
    fn resolves_relative_paths() {
        let verbatim = to_verbatim(Path::new("relative.txt")).expect("verbatim");

        assert!(is_verbatim(&verbatim));
        assert!(verbatim.to_string_lossy().ends_with("relative.txt"));
    }

    #[test]
    fn preserves_unicode_names() {
        let path = Path::new(r"C:\Данные\Отчёт.txt");
        let verbatim = to_verbatim(path).expect("verbatim");

        assert_eq!(strip_verbatim(&verbatim), path);
        assert_eq!(display(&verbatim), r"C:\Данные\Отчёт.txt");
    }

    #[test]
    fn rejects_paths_over_the_limit() {
        let long = format!(r"C:\{}", "a".repeat(MAX_PATH_CHARS));
        let error = validate_length(Path::new(&long)).expect_err("path must be rejected");

        assert_eq!(error.code().as_str(), "E_INVALID_PATH");
    }

    #[test]
    fn trims_trailing_separators_but_keeps_roots() {
        let normalized = normalize(Path::new(r"C:\temp\dir\")).expect("normalize");

        assert_eq!(normalized, PathBuf::from(r"\\?\C:\temp\dir"));

        let root = normalize(Path::new(r"C:\")).expect("normalize root");
        assert_eq!(root, PathBuf::from(r"\\?\C:\"));
    }

    #[test]
    fn finds_the_volume_root() {
        let directory = tempfile::tempdir().expect("temp dir");
        let nested = directory.path().join("a").join("b");

        assert_eq!(
            volume_root(&nested).expect("volume root"),
            volume_root(directory.path()).expect("volume root")
        );
    }

    #[test]
    fn enforces_the_selected_scope() {
        let directory = tempfile::tempdir().expect("temp dir");
        let root = directory.path();
        let child = root.join("allowed").join("file.txt");

        ensure_within(root, &child).expect("child is inside");
        ensure_within(root, root).expect("root is inside itself");

        let outside = root.parent().expect("parent").join("outside.txt");
        let error = ensure_within(root, &outside).expect_err("must reject");

        assert_eq!(error.code().as_str(), "E_OPERATION");
    }
}
