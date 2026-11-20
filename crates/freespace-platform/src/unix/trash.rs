//! XDG trash (freedesktop.org specification).
//!
//! A file on the same volume as `$XDG_DATA_HOME/Trash` goes to that home trash;
//! a file on another volume goes to `$topdir/.Trash/$uid` or
//! `$topdir/.Trash-$uid`. Every move writes a `.trashinfo` record first, so a
//! file manager can restore the object, and then renames the object inside the
//! volume. Nothing is ever deleted when the trash itself cannot be prepared.
//!
//! The application never restores from the trash; that is the job of the
//! desktop file manager, as the specification intends.

use std::{
    env,
    ffi::{OsStr, OsString},
    fs, io,
    os::unix::{ffi::OsStrExt as _, fs::MetadataExt as _},
    path::{Path, PathBuf},
};

use freespace_core::{Error, Result, Timestamp};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_encode};
use rustix::process::geteuid;

use super::paths;

/// Characters kept literal in the `Path=` field; everything else is escaped.
///
/// The unreserved set `A-Z a-z 0-9 - . _ ~` and the path separator stay as they
/// are, matching the URI form the freedesktop specification requires.
const TRASH_PATH_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'/')
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// `files/` and `info/` directories of one trash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TrashPaths {
    /// Directory that receives the objects themselves.
    pub files: PathBuf,
    /// Directory that receives the `.trashinfo` records.
    pub info: PathBuf,
}

impl TrashPaths {
    /// Builds the pair below one trash root.
    pub(crate) fn new(root: &Path) -> Self {
        Self {
            files: root.join("files"),
            info: root.join("info"),
        }
    }
}

/// Moves `targets` into the XDG trash and returns how many were moved.
///
/// # Errors
/// Returns an error before changing data when no trash can be prepared for a
/// target volume, and propagates the first move failure. There is deliberately
/// no silent fallback to permanent deletion.
pub fn trash(targets: &[PathBuf]) -> Result<u32> {
    if targets.is_empty() {
        return Ok(0);
    }

    let home = home_trash_paths()?;
    let home_volume = paths::volume_root(&home.files)?;
    let uid = geteuid().as_raw();
    let deletion_date = deletion_date();
    let mut moved = 0;

    for target in targets {
        let volume = paths::volume_root(target)?;
        let trash = if volume == home_volume {
            home.clone()
        } else {
            volume_trash_paths(&volume, uid)?
        };
        move_to_trash(target, &trash, &deletion_date)?;
        moved += 1;
    }

    Ok(moved)
}

/// Returns the home trash below `$XDG_DATA_HOME` (or `~/.local/share`).
pub(crate) fn home_trash_paths() -> Result<TrashPaths> {
    if let Some(data_home) = env::var_os("XDG_DATA_HOME") {
        let data_home = PathBuf::from(data_home);
        if data_home.is_absolute() {
            return Ok(TrashPaths::new(&data_home.join("Trash")));
        }
    }

    let home = env::var_os("HOME").filter(|value| !value.is_empty());
    let Some(home) = home else {
        return Err(Error::Unsupported(
            "the XDG trash needs XDG_DATA_HOME or HOME".to_owned(),
        ));
    };

    Ok(TrashPaths::new(
        &PathBuf::from(home).join(".local/share/Trash"),
    ))
}

/// Returns the trash of a volume that is not the home volume.
///
/// The shared `$topdir/.Trash/$uid` is used only when the shared directory
/// already exists and is safe; otherwise the private `$topdir/.Trash-$uid` is
/// created. Creating the private directory is the first write of the operation.
pub(crate) fn volume_trash_paths(volume: &Path, uid: u32) -> Result<TrashPaths> {
    if let Some(shared) = safe_shared_trash(volume, uid) {
        let paths = TrashPaths::new(&shared);
        ensure_directories(&paths)?;

        return Ok(paths);
    }

    let root = volume.join(format!(".Trash-{uid}"));
    let paths = TrashPaths::new(&root);
    ensure_directories(&paths)?;

    Ok(paths)
}

/// Returns `$topdir/.Trash/$uid` when the shared directory is safe to use.
fn safe_shared_trash(volume: &Path, uid: u32) -> Option<PathBuf> {
    let shared = volume.join(".Trash");
    let metadata = fs::symlink_metadata(&shared).ok()?;

    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return None;
    }
    let owner = metadata.uid();
    if owner == uid {
        return Some(shared.join(uid.to_string()));
    }
    // A root-owned shared trash must be sticky and world-writable, as required
    // by the specification; otherwise another user could hijack the path.
    let mode = metadata.mode();
    if owner == 0 && mode & 0o1000 != 0 && mode & 0o002 != 0 {
        return Some(shared.join(uid.to_string()));
    }

    None
}

fn ensure_directories(paths: &TrashPaths) -> Result<()> {
    for directory in [&paths.files, &paths.info] {
        fs::create_dir_all(directory).map_err(|error| map_io(directory, error))?;
    }

    Ok(())
}

/// Moves one object into `trash` and returns its new path.
pub(crate) fn move_to_trash(
    target: &Path,
    trash: &TrashPaths,
    deletion_date: &str,
) -> Result<PathBuf> {
    ensure_directories(trash)?;

    let name = unique_name(
        target.file_name().ok_or_else(|| {
            Error::InvalidArgument(format!(
                "`{}` has no file name and cannot be trashed",
                paths::display(target)
            ))
        })?,
        &trash.files,
        &trash.info,
    );
    let destination = trash.files.join(&name);
    let mut record_name = name.clone();
    record_name.push(".trashinfo");
    let information = trash.info.join(record_name);
    let record = trashinfo_record(target, deletion_date)?;

    fs::write(&information, record).map_err(|error| map_io(&information, error))?;
    if let Err(error) = fs::rename(target, &destination) {
        // The record without an object would be a ghost; remove it again.
        let _ = fs::remove_file(&information);
        return Err(map_io(target, error));
    }

    Ok(destination)
}

/// Finds a free name inside `files` and `info`, appending `.2`, `.3`, …
///
/// Names that are not valid UTF-8 cannot be split into a stem and extension;
/// they keep their bytes and receive a counter suffix as a whole.
pub(crate) fn unique_name(name: &OsStr, files: &Path, info: &Path) -> OsString {
    if !is_taken(name, files, info) {
        return name.to_os_string();
    }

    let Some(text) = name.to_str() else {
        for counter in 2_u32.. {
            let mut candidate = name.to_os_string();
            candidate.push(format!(".{counter}"));
            if !is_taken(&candidate, files, info) {
                return candidate;
            }
        }

        unreachable!("the counter space is large enough for every real directory")
    };

    let (stem, extension) = split_extension(text);
    for counter in 2_u32.. {
        let candidate = match extension {
            Some(extension) => format!("{stem}.{counter}.{extension}"),
            None => format!("{stem}.{counter}"),
        };
        if !is_taken(OsStr::new(&candidate), files, info) {
            return OsString::from(candidate);
        }
    }

    unreachable!("the counter space is large enough for every real directory")
}

fn is_taken(name: &OsStr, files: &Path, info: &Path) -> bool {
    if files.join(name).exists() {
        return true;
    }
    let mut record_name = name.to_os_string();
    record_name.push(".trashinfo");

    info.join(record_name).exists()
}

/// Splits `name` into a stem and the final extension.
///
/// Dot files such as `.profile` have no extension, and trailing dots are part
/// of the stem so the generated name stays readable.
fn split_extension(name: &str) -> (&str, Option<&str>) {
    if name.starts_with('.') && name[1..].find('.').is_none() {
        return (name, None);
    }

    match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() && !extension.is_empty() => {
            (stem, Some(extension))
        }
        _ => (name, None),
    }
}

/// Builds the `.trashinfo` content for one absolute path.
pub(crate) fn trashinfo_record(target: &Path, deletion_date: &str) -> Result<String> {
    let absolute = paths::normalize(target)?;
    let encoded = percent_encode(absolute.as_os_str().as_bytes(), TRASH_PATH_SET);

    Ok(format!(
        "[Trash Info]\nPath={encoded}\nDeletionDate={deletion_date}\n"
    ))
}

/// Current deletion time in the `YYYY-MM-DDThh:mm:ss` form the spec requires.
fn deletion_date() -> String {
    let value = Timestamp::now().format_iso8601();
    value.strip_suffix('Z').unwrap_or(&value).to_owned()
}

fn map_io(path: &Path, error: io::Error) -> Error {
    match error.kind() {
        io::ErrorKind::NotFound => Error::NotFound(paths::display(path)),
        io::ErrorKind::PermissionDenied => Error::AccessDenied(paths::display(path)),
        io::ErrorKind::AlreadyExists => Error::Conflict(paths::display(path)),
        _ => Error::Io(error),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        ffi::{OsStr, OsString},
        os::unix::ffi::{OsStrExt as _, OsStringExt as _},
        path::Path,
    };

    use super::{TrashPaths, move_to_trash, trashinfo_record, unique_name, volume_trash_paths};

    #[test]
    fn writes_a_percent_encoded_record_with_the_deletion_date() {
        let record = trashinfo_record(Path::new("/tmp/a b/report #1.txt"), "2026-11-20T10:11:12")
            .expect("record");

        assert!(record.starts_with("[Trash Info]\n"));
        assert!(record.contains("Path=/tmp/a%20b/report%20%231.txt\n"));
        assert!(record.contains("DeletionDate=2026-11-20T10:11:12\n"));
        assert!(record.ends_with('\n'));
    }

    #[test]
    fn moves_an_object_and_writes_its_record() {
        let directory = tempfile::tempdir().expect("temp dir");
        let trash = TrashPaths::new(&directory.path().join("Trash"));
        let source = directory.path().join("report.txt");
        std::fs::write(&source, b"payload").expect("write fixture");

        let destination =
            move_to_trash(&source, &trash, "2026-11-20T10:11:12").expect("move to trash");

        assert!(!source.exists(), "the original must be gone");
        assert_eq!(destination, trash.files.join("report.txt"));
        assert_eq!(
            std::fs::read(destination).expect("read trashed object"),
            b"payload"
        );
        let record =
            std::fs::read_to_string(trash.info.join("report.txt.trashinfo")).expect("read record");
        assert!(record.contains("Path="));
    }

    #[test]
    fn resolves_name_collisions_before_the_extension() {
        let directory = tempfile::tempdir().expect("temp dir");
        let trash = TrashPaths::new(&directory.path().join("Trash"));
        std::fs::create_dir_all(&trash.files).expect("files");
        std::fs::create_dir_all(&trash.info).expect("info");
        std::fs::write(trash.files.join("archive.tar.gz"), b"first").expect("occupied");

        assert_eq!(
            unique_name(OsStr::new("archive.tar.gz"), &trash.files, &trash.info),
            OsString::from("archive.tar.2.gz")
        );
        std::fs::write(trash.files.join("archive.tar.2.gz"), b"second").expect("occupied");
        assert_eq!(
            unique_name(OsStr::new("archive.tar.gz"), &trash.files, &trash.info),
            OsString::from("archive.tar.3.gz")
        );
    }

    #[test]
    fn keeps_dot_files_whole() {
        let directory = tempfile::tempdir().expect("temp dir");
        let files = directory.path().join("files");
        let info = directory.path().join("info");
        std::fs::create_dir_all(&files).expect("files");
        std::fs::create_dir_all(&info).expect("info");
        std::fs::write(files.join(".profile"), b"occupied").expect("occupied");

        assert_eq!(
            unique_name(OsStr::new(".profile"), &files, &info),
            OsString::from(".profile.2")
        );
    }

    #[test]
    fn keeps_non_utf8_names_and_only_appends_a_counter() {
        let directory = tempfile::tempdir().expect("temp dir");
        let files = directory.path().join("files");
        let info = directory.path().join("info");
        std::fs::create_dir_all(&files).expect("files");
        std::fs::create_dir_all(&info).expect("info");
        let name = OsString::from_vec(b"bad\xFF.bin".to_vec());
        std::fs::write(files.join(&name), b"occupied").expect("occupied");

        let unique = unique_name(&name, &files, &info);

        assert_eq!(unique.as_os_str().as_bytes(), b"bad\xFF.bin.2");
    }

    #[test]
    fn creates_the_private_volume_trash_when_no_shared_one_exists() {
        let directory = tempfile::tempdir().expect("temp dir");
        let volume = directory.path();

        let paths = volume_trash_paths(volume, 4242).expect("private trash");

        assert_eq!(paths.files, volume.join(".Trash-4242").join("files"));
        assert!(paths.files.is_dir());
        assert!(paths.info.is_dir());
    }

    #[test]
    fn prefers_a_shared_trash_directory_owned_by_the_user() {
        let directory = tempfile::tempdir().expect("temp dir");
        let volume = directory.path();
        std::fs::create_dir(volume.join(".Trash")).expect("shared trash");
        let uid = rustix::process::geteuid().as_raw();

        let paths = volume_trash_paths(volume, uid).expect("shared trash");

        assert_eq!(
            paths.files,
            volume.join(".Trash").join(uid.to_string()).join("files")
        );
    }
}
