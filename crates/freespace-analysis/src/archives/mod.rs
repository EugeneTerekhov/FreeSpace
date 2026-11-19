//! Archive inspection without unpacking (FR-066…FR-069).
//!
//! Supported containers: ZIP, 7z, TAR, TAR/GZ and ISO. RAR is available only
//! when the `rar` feature is compiled in and a compatible library is present;
//! see [`rar`].
//!
//! Every reader returns the same [`ArchiveEntry`] shape so the CLI, the GUI and
//! the report writer share one schema. Entries are never extracted to disk.

pub mod limits;
pub mod rar;

use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use freespace_core::{Error, Result};
use serde::{Deserialize, Serialize};

pub use limits::ArchiveLimits;

/// Container format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArchiveFormat {
    /// ZIP and ZIP64.
    Zip,
    /// 7-Zip.
    SevenZip,
    /// Uncompressed TAR.
    Tar,
    /// Gzip-compressed TAR.
    TarGz,
    /// ISO 9660 image.
    Iso,
    /// RAR, when the optional reader is available.
    Rar,
}

impl ArchiveFormat {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Zip => "zip",
            Self::SevenZip => "7z",
            Self::Tar => "tar",
            Self::TarGz => "tar.gz",
            Self::Iso => "iso",
            Self::Rar => "rar",
        }
    }

    /// Detects the format from a file name.
    #[must_use]
    pub fn for_path(path: &Path) -> Option<Self> {
        let name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
        if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
            return Some(Self::TarGz);
        }
        if name.ends_with(".tar.bz2") || name.ends_with(".tar.xz") {
            return None;
        }

        match path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "zip" | "jar" | "docx" | "xlsx" | "pptx" | "apk" => Some(Self::Zip),
            "7z" => Some(Self::SevenZip),
            "tar" => Some(Self::Tar),
            "gz" => Some(Self::TarGz),
            "iso" | "img" => Some(Self::Iso),
            "rar" => Some(Self::Rar),
            _ => None,
        }
    }
}

/// One file or directory inside an archive (data-model §19).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchiveEntry {
    /// Path inside the archive, using `/` as the separator.
    pub internal_path: String,
    /// Uncompressed size in bytes; zero for directories.
    pub size: u64,
    /// Compressed size, when the container reports it.
    pub compressed_size: Option<u64>,
    /// Whether the entry is a directory.
    pub is_directory: bool,
    /// Whether the entry is encrypted.
    pub encrypted: bool,
    /// Share of the archive's uncompressed content, in `0.0..=1.0`.
    pub share: f64,
}

impl ArchiveEntry {
    /// Builds a regular file entry.
    #[must_use]
    pub fn file(internal_path: impl Into<String>, size: u64, compressed_size: Option<u64>) -> Self {
        Self {
            internal_path: internal_path.into(),
            size,
            compressed_size,
            is_directory: false,
            encrypted: false,
            share: 0.0,
        }
    }

    /// Builds a directory entry.
    #[must_use]
    pub fn directory(internal_path: impl Into<String>) -> Self {
        Self {
            internal_path: internal_path.into(),
            size: 0,
            compressed_size: None,
            is_directory: true,
            encrypted: false,
            share: 0.0,
        }
    }
}

/// Result of listing one archive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchiveInspection {
    /// Archive file.
    pub archive: PathBuf,
    /// Detected format.
    pub format: ArchiveFormat,
    /// Entries in container order.
    pub entries: Vec<ArchiveEntry>,
    /// Total uncompressed file size.
    pub total_size: u64,
    /// Number of entries whose content is encrypted.
    pub encrypted_count: u64,
    /// Non-fatal notes, for example an encrypted member.
    pub warnings: Vec<String>,
}

impl ArchiveInspection {
    /// Number of regular files.
    #[must_use]
    pub fn file_count(&self) -> u64 {
        self.entries
            .iter()
            .filter(|entry| !entry.is_directory)
            .count()
            .try_into()
            .unwrap_or(u64::MAX)
    }

    /// Returns the largest entries, sorted by descending size.
    #[must_use]
    pub fn largest(&self, limit: usize) -> Vec<&ArchiveEntry> {
        let mut entries: Vec<&ArchiveEntry> = self
            .entries
            .iter()
            .filter(|entry| !entry.is_directory)
            .collect();
        entries.sort_by_key(|entry| std::cmp::Reverse(entry.size));
        entries.truncate(limit);

        entries
    }
}

/// Lists an archive without extracting it.
///
/// # Errors
/// Returns [`Error::Unsupported`] for unknown formats, password-protected or
/// bomb-like archives, and [`Error::Operation`] for corrupted containers.
pub fn inspect(path: &Path, limits: ArchiveLimits) -> Result<ArchiveInspection> {
    let metadata = std::fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(Error::InvalidArgument(format!(
            "`{}` is not an archive file",
            path.display()
        )));
    }

    let format = detect_format(path)?;
    let mut entries = match format {
        ArchiveFormat::Zip => read_zip(path)?,
        ArchiveFormat::SevenZip => read_seven_zip(path)?,
        ArchiveFormat::Tar => read_tar(path, false)?,
        ArchiveFormat::TarGz => read_tar(path, true)?,
        ArchiveFormat::Iso => read_iso(path)?,
        ArchiveFormat::Rar => rar::read_rar(path)?,
    };

    limits.check_count(entries.len())?;
    for entry in &entries {
        limits.check_entry(entry)?;
    }

    let total_size = entries
        .iter()
        .filter(|entry| !entry.is_directory)
        .map(|entry| entry.size)
        .fold(0u64, u64::saturating_add);
    limits.check_totals(total_size, metadata.len())?;

    for entry in &mut entries {
        entry.internal_path = entry.internal_path.replace('\\', "/");
        entry.share = if total_size == 0 {
            0.0
        } else {
            entry.size as f64 / total_size as f64
        };
    }

    let encrypted_count = entries.iter().filter(|entry| entry.encrypted).count() as u64;
    let mut warnings = Vec::new();
    if encrypted_count > 0 {
        warnings.push(format!(
            "{encrypted_count} archive entries are encrypted; their contents were not read"
        ));
    }

    Ok(ArchiveInspection {
        archive: path.to_path_buf(),
        format,
        entries,
        total_size,
        encrypted_count,
        warnings,
    })
}

/// Finds archive files below `root`.
///
/// Hidden directories are still visited; the caller decides what to show. The
/// result is capped so a directory with thousands of archives cannot stall the
/// UI.
#[must_use]
pub fn find_archives(root: &Path, limit: usize) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(directory) = stack.pop() {
        if found.len() >= limit {
            break;
        }
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                stack.push(path);
            } else if file_type.is_file() && ArchiveFormat::for_path(&path).is_some() {
                found.push(path);
                if found.len() >= limit {
                    break;
                }
            }
        }
    }

    found.sort();

    found
}

fn detect_format(path: &Path) -> Result<ArchiveFormat> {
    if let Some(format) = ArchiveFormat::for_path(path) {
        return Ok(format);
    }

    let mut file = File::open(path)?;
    let mut header = [0u8; 512];
    let read = file.read(&mut header)?;

    if read >= 4 && header.starts_with(b"PK\x03\x04") {
        return Ok(ArchiveFormat::Zip);
    }
    if read >= 6 && header.starts_with(b"7z\xBC\xAF\x27\x1C") {
        return Ok(ArchiveFormat::SevenZip);
    }
    if read >= 8 && header.starts_with(b"Rar!\x1A\x07") {
        return Ok(ArchiveFormat::Rar);
    }
    if read >= 2 && header.starts_with(&[0x1F, 0x8B]) {
        return Ok(ArchiveFormat::TarGz);
    }
    if read >= 262 && &header[257..262] == b"ustar" {
        return Ok(ArchiveFormat::Tar);
    }
    if is_iso(&mut file)? {
        return Ok(ArchiveFormat::Iso);
    }

    Err(Error::Unsupported(format!(
        "`{}` is not a supported archive",
        path.display()
    )))
}

fn is_iso(file: &mut File) -> Result<bool> {
    let mut magic = [0u8; 6];
    file.seek(SeekFrom::Start(0x8001))?;
    if file.read(&mut magic)? != magic.len() {
        return Ok(false);
    }

    Ok(header_is_iso(&magic))
}

fn header_is_iso(magic: &[u8]) -> bool {
    magic.starts_with(b"CD001")
}

fn read_zip(path: &Path) -> Result<Vec<ArchiveEntry>> {
    let file = File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|error| Error::Operation(format!("corrupted ZIP archive: {error}")))?;
    let mut entries = Vec::with_capacity(archive.len());

    for index in 0..archive.len() {
        let entry = archive
            .by_index_raw(index)
            .map_err(|error| Error::Operation(format!("corrupted ZIP entry: {error}")))?;
        let name = entry
            .name()
            .map_err(|error| Error::Operation(format!("invalid ZIP entry name: {error}")))?
            .into_owned();
        let mut item = if entry.is_dir() {
            ArchiveEntry::directory(name)
        } else {
            ArchiveEntry::file(name, entry.size(), Some(entry.compressed_size()))
        };
        item.encrypted = entry.encrypted();
        entries.push(item);
    }

    Ok(entries)
}

fn read_seven_zip(path: &Path) -> Result<Vec<ArchiveEntry>> {
    let reader =
        sevenz_rust::SevenZReader::open(path, sevenz_rust::Password::empty()).map_err(|error| {
            let message = error.to_string();
            if message.to_ascii_lowercase().contains("password")
                || message.to_ascii_lowercase().contains("encrypt")
            {
                Error::Unsupported(format!("the 7z archive is password-protected: {message}"))
            } else {
                Error::Operation(format!("corrupted 7z archive: {message}"))
            }
        })?;

    Ok(reader
        .archive()
        .files
        .iter()
        .map(|entry| {
            if entry.is_directory {
                ArchiveEntry::directory(entry.name.clone())
            } else {
                ArchiveEntry::file(entry.name.clone(), entry.size, Some(entry.compressed_size))
            }
        })
        .collect())
}

fn read_tar(path: &Path, gzip: bool) -> Result<Vec<ArchiveEntry>> {
    let file = File::open(path)?;
    let reader: Box<dyn Read> = if gzip {
        Box::new(flate2::read::GzDecoder::new(BufReader::new(file)))
    } else {
        Box::new(BufReader::new(file))
    };
    let mut archive = tar::Archive::new(reader);
    let entries = archive
        .entries()
        .map_err(|error| Error::Operation(format!("corrupted TAR archive: {error}")))?;
    let mut result = Vec::new();

    for entry in entries {
        let entry =
            entry.map_err(|error| Error::Operation(format!("corrupted TAR entry: {error}")))?;
        let header = entry.header();
        let path = entry
            .path()
            .map_err(|error| Error::Operation(format!("invalid TAR entry name: {error}")))?
            .to_string_lossy()
            .into_owned();
        let is_directory = header.entry_type().is_dir();
        if is_directory {
            result.push(ArchiveEntry::directory(path));
        } else {
            let size = header
                .size()
                .map_err(|error| Error::Operation(format!("invalid TAR entry size: {error}")))?;
            result.push(ArchiveEntry::file(path, size, None));
        }
    }

    Ok(result)
}

fn read_iso(path: &Path) -> Result<Vec<ArchiveEntry>> {
    let file = File::open(path)?;
    let iso = iso9660::ISO9660::new(file)
        .map_err(|error| Error::Operation(format!("corrupted ISO image: {error}")))?;
    let mut entries = Vec::new();
    walk_iso(&iso.root, "", &mut entries, 0)?;

    Ok(entries)
}

fn walk_iso(
    directory: &iso9660::ISODirectory<File>,
    prefix: &str,
    entries: &mut Vec<ArchiveEntry>,
    depth: usize,
) -> Result<()> {
    if depth > 64 {
        return Err(Error::Unsupported(
            "ISO directory nesting is deeper than 64 levels".to_owned(),
        ));
    }

    for entry in directory.contents() {
        let entry =
            entry.map_err(|error| Error::Operation(format!("corrupted ISO entry: {error}")))?;
        let name = entry.identifier();
        if name == "." || name == ".." || name.is_empty() {
            continue;
        }
        let internal_path = if prefix.is_empty() {
            name.to_owned()
        } else {
            format!("{prefix}/{name}")
        };

        match entry {
            iso9660::DirectoryEntry::Directory(child) => {
                entries.push(ArchiveEntry::directory(internal_path.clone()));
                walk_iso(&child, &internal_path, entries, depth + 1)?;
            }
            iso9660::DirectoryEntry::File(file) => {
                entries.push(ArchiveEntry::file(
                    internal_path,
                    u64::from(file.size()),
                    None,
                ));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        ArchiveFormat, ArchiveLimits, detect_format, find_archives, header_is_iso, inspect,
    };

    #[test]
    fn detects_formats_by_extension_and_magic() {
        assert_eq!(
            ArchiveFormat::for_path(Path::new("photos.zip")),
            Some(ArchiveFormat::Zip)
        );
        assert_eq!(
            ArchiveFormat::for_path(Path::new("backup.TAR.GZ")),
            Some(ArchiveFormat::TarGz)
        );
        assert_eq!(
            ArchiveFormat::for_path(Path::new("image.iso")),
            Some(ArchiveFormat::Iso)
        );
        assert!(header_is_iso(b"CD001\0"));
        assert!(!header_is_iso(b"CD002\0"));
    }

    #[test]
    fn rejects_unsupported_files() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("notes.txt");
        std::fs::write(&path, b"plain text").expect("write");

        let error = inspect(&path, ArchiveLimits::default()).expect_err("unsupported");
        assert_eq!(error.code().as_str(), "E_UNSUPPORTED");
        assert!(detect_format(&path).is_err());
    }

    #[test]
    fn finds_archives_below_a_directory() {
        let directory = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(directory.path().join("nested")).expect("nested");
        std::fs::write(directory.path().join("a.zip"), b"PK\x03\x04").expect("a");
        std::fs::write(directory.path().join("nested").join("b.tar"), b"ustar").expect("b");
        std::fs::write(directory.path().join("c.txt"), b"text").expect("c");

        let found = find_archives(directory.path(), 10);
        assert_eq!(found.len(), 2);
    }
}
