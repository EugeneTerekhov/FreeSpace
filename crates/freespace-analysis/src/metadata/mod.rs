//! File metadata extraction for EXIF, audio, video and PE version resources
//! (FR-061…FR-063).
//!
//! Extraction is best effort: an unsupported file kind, a damaged container or
//! a missing tag never fails the analysis. Every record carries a
//! [`MetadataStatus`] so the UI can show `н/д` and continue.
//!
//! [`MetadataCache`] keeps the last result per path and invalidates it when the
//! modification time changes, which makes repeated filtering inexpensive.

pub mod exif;
pub mod media;
pub mod pe;

use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    sync::Mutex,
};

use freespace_core::{
    Result, Timestamp,
    tree::{FsTree, NodeId, NodeKind},
};
use serde::{Deserialize, Serialize};

/// Well-known tag keys. Values are strings so every container can share the
/// same [`MetadataRecord`] shape.
pub mod tags {
    /// Image or video width in pixels.
    pub const WIDTH: &str = "Width";
    /// Image or video height in pixels.
    pub const HEIGHT: &str = "Height";
    /// Duration in whole seconds.
    pub const DURATION_SECS: &str = "DurationSecs";
    /// Audio bitrate in kilobits per second.
    pub const BITRATE_KBPS: &str = "BitrateKbps";
    /// Audio sample rate in hertz.
    pub const SAMPLE_RATE: &str = "SampleRate";
    /// Audio channel count.
    pub const CHANNELS: &str = "Channels";
    /// Codec or container name.
    pub const CODEC: &str = "Codec";
    /// Camera make.
    pub const CAMERA_MAKE: &str = "CameraMake";
    /// Camera model.
    pub const CAMERA_MODEL: &str = "CameraModel";
    /// Original capture time as stored in EXIF.
    pub const TAKEN_AT: &str = "TakenAt";
    /// PE file version.
    pub const FILE_VERSION: &str = "FileVersion";
    /// PE product version.
    pub const PRODUCT_VERSION: &str = "ProductVersion";
    /// PE product name.
    pub const PRODUCT_NAME: &str = "ProductName";
    /// PE file description.
    pub const FILE_DESCRIPTION: &str = "FileDescription";
    /// PE company name.
    pub const COMPANY_NAME: &str = "CompanyName";
}

/// Metadata family of a file.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MetadataKind {
    /// Exchangeable image file format.
    #[default]
    Exif,
    /// Audio container tags and stream properties.
    Audio,
    /// ISO base media video properties.
    Video,
    /// Portable Executable version resources.
    Pe,
}

impl MetadataKind {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Exif => "exif",
            Self::Audio => "audio",
            Self::Video => "video",
            Self::Pe => "pe",
        }
    }

    /// Parses a machine-readable name.
    #[must_use]
    pub fn parse_value(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "exif" => Some(Self::Exif),
            "audio" => Some(Self::Audio),
            "video" => Some(Self::Video),
            "pe" => Some(Self::Pe),
            _ => None,
        }
    }

    /// Returns the metadata family implied by a file extension.
    #[must_use]
    pub fn for_path(path: &Path) -> Option<Self> {
        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();

        match extension.as_str() {
            "jpg" | "jpeg" | "jpe" | "tif" | "tiff" | "heic" | "heif" | "webp" | "dng" | "cr2"
            | "nef" | "arw" | "rw2" => Some(Self::Exif),
            "mp3" | "flac" | "wav" | "wave" | "ogg" | "oga" | "opus" | "m4a" | "aac" | "wma"
            | "aiff" | "aif" | "ape" | "wv" => Some(Self::Audio),
            "mp4" | "m4v" | "mov" | "3gp" | "3g2" => Some(Self::Video),
            "exe" | "dll" | "sys" | "ocx" | "cpl" | "scr" | "drv" | "efi" | "mui" => Some(Self::Pe),
            _ => None,
        }
    }
}

/// Result of one extraction attempt.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MetadataStatus {
    /// The file was parsed successfully.
    #[default]
    Ok,
    /// The extension is not supported.
    Unsupported,
    /// The container exists but could not be parsed.
    Corrupted,
}

impl MetadataStatus {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Unsupported => "unsupported",
            Self::Corrupted => "corrupted",
        }
    }
}

/// One extracted metadata record (data-model §17).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataRecord {
    /// Node inside the analysed tree.
    pub node_id: NodeId,
    /// Metadata family.
    pub kind: MetadataKind,
    /// Extracted tags.
    pub tags: BTreeMap<String, String>,
    /// Extraction time.
    pub read_at: Timestamp,
    /// Extraction result.
    pub status: MetadataStatus,
    /// Diagnostic message for corrupted files; never shown as a tag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl MetadataRecord {
    /// Builds a record with an explicit status.
    #[must_use]
    pub fn new(node_id: NodeId, kind: MetadataKind, status: MetadataStatus) -> Self {
        Self {
            node_id,
            kind,
            tags: BTreeMap::new(),
            read_at: Timestamp::now(),
            status,
            error: None,
        }
    }

    /// Returns one tag.
    #[must_use]
    pub fn tag(&self, key: &str) -> Option<&str> {
        self.tags.get(key).map(String::as_str)
    }

    /// Adds a tag.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.tags.insert(key.into(), value.into());
    }

    /// Parses an integer tag.
    #[must_use]
    pub fn number(&self, key: &str) -> Option<u64> {
        self.tag(key)?.trim().parse().ok()
    }

    /// Image or video width.
    #[must_use]
    pub fn width(&self) -> Option<u32> {
        self.number(tags::WIDTH)
            .and_then(|value| u32::try_from(value).ok())
    }

    /// Image or video height.
    #[must_use]
    pub fn height(&self) -> Option<u32> {
        self.number(tags::HEIGHT)
            .and_then(|value| u32::try_from(value).ok())
    }

    /// Duration in seconds.
    #[must_use]
    pub fn duration_secs(&self) -> Option<u64> {
        self.number(tags::DURATION_SECS)
    }

    /// Audio bitrate in kilobits per second.
    #[must_use]
    pub fn bitrate_kbps(&self) -> Option<u32> {
        self.number(tags::BITRATE_KBPS)
            .and_then(|value| u32::try_from(value).ok())
    }

    /// Returns `true` when the record can be displayed.
    #[must_use]
    pub const fn is_available(&self) -> bool {
        matches!(self.status, MetadataStatus::Ok)
    }
}

/// Extracts metadata for `path`, attributing it to `node_id`.
///
/// # Errors
/// This function never fails: unreadable, unsupported and damaged files are
/// reported through [`MetadataStatus`] so analysis can continue (FR-063).
#[must_use]
pub fn extract(node_id: NodeId, path: &Path) -> MetadataRecord {
    let Some(kind) = MetadataKind::for_path(path) else {
        return MetadataRecord::new(node_id, MetadataKind::Exif, MetadataStatus::Unsupported);
    };

    match read_tags(kind, path) {
        Ok(tags) => {
            let mut record = MetadataRecord::new(node_id, kind, MetadataStatus::Ok);
            record.tags = tags;
            record
        }
        Err(error) => {
            let mut record = MetadataRecord::new(node_id, kind, MetadataStatus::Corrupted);
            record.error = Some(error.to_string());
            record
        }
    }
}

/// Dispatches to the family-specific reader.
///
/// # Errors
/// Returns an error when the container cannot be opened or parsed.
pub fn read_tags(kind: MetadataKind, path: &Path) -> Result<BTreeMap<String, String>> {
    match kind {
        MetadataKind::Exif => exif::read(path),
        MetadataKind::Audio => media::read_audio(path),
        MetadataKind::Video => media::read_video(path),
        MetadataKind::Pe => pe::read(path),
    }
}

/// Filters metadata records (FR-062).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataFilter {
    /// Accepted metadata families; empty means every family.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kinds: Vec<MetadataKind>,
    /// Minimum width in pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_width: Option<u32>,
    /// Minimum height in pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_height: Option<u32>,
    /// Minimum audio bitrate in kbps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_bitrate_kbps: Option<u32>,
    /// Minimum duration in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_duration_secs: Option<u64>,
    /// Case-insensitive camera model substring.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<String>,
    /// Case-insensitive codec substring.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codec: Option<String>,
}

impl MetadataFilter {
    /// Returns `true` when the filter constrains anything.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty()
            && self.min_width.is_none()
            && self.min_height.is_none()
            && self.min_bitrate_kbps.is_none()
            && self.min_duration_secs.is_none()
            && self.camera.is_none()
            && self.codec.is_none()
    }

    /// Returns `true` when a record satisfies every constraint.
    ///
    /// Unsupported or corrupted records do not match any metadata constraint.
    #[must_use]
    pub fn matches(&self, record: &MetadataRecord) -> bool {
        if !record.is_available() {
            return false;
        }
        if !self.kinds.is_empty() && !self.kinds.contains(&record.kind) {
            return false;
        }
        if self
            .min_width
            .is_some_and(|minimum| record.width().is_none_or(|value| value < minimum))
        {
            return false;
        }
        if self
            .min_height
            .is_some_and(|minimum| record.height().is_none_or(|value| value < minimum))
        {
            return false;
        }
        if self
            .min_bitrate_kbps
            .is_some_and(|minimum| record.bitrate_kbps().is_none_or(|value| value < minimum))
        {
            return false;
        }
        if self
            .min_duration_secs
            .is_some_and(|minimum| record.duration_secs().is_none_or(|value| value < minimum))
        {
            return false;
        }
        if let Some(camera) = &self.camera {
            let camera = camera.to_lowercase();
            let matches_camera = record
                .tag(tags::CAMERA_MODEL)
                .into_iter()
                .chain(record.tag(tags::CAMERA_MAKE))
                .any(|value| value.to_lowercase().contains(&camera));
            if !matches_camera {
                return false;
            }
        }
        if let Some(codec) = &self.codec {
            let codec = codec.to_lowercase();
            if record
                .tag(tags::CODEC)
                .is_none_or(|value| !value.to_lowercase().contains(&codec))
            {
                return false;
            }
        }

        true
    }
}

/// Cache entry keyed by path and invalidated by modification time.
#[derive(Debug)]
struct CachedRecord {
    modified: Option<Timestamp>,
    record: MetadataRecord,
}

/// In-memory metadata cache with background-friendly batch extraction.
#[derive(Debug, Default)]
pub struct MetadataCache {
    entries: Mutex<HashMap<PathBuf, CachedRecord>>,
}

impl MetadataCache {
    /// Creates an empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of cached paths.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries
            .lock()
            .map(|entries| entries.len())
            .unwrap_or_default()
    }

    /// Returns `true` when nothing is cached.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Removes every cached record.
    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.clear();
        }
    }

    /// Extracts metadata, reusing the cached record while the file is unchanged.
    pub fn extract(&self, node_id: NodeId, path: &Path) -> MetadataRecord {
        let modified = std::fs::metadata(path)
            .ok()
            .map(|metadata| Timestamp::from(metadata.modified().unwrap_or(std::time::UNIX_EPOCH)));
        if let Ok(entries) = self.entries.lock() {
            if let Some(cached) = entries.get(path) {
                if cached.modified == modified {
                    let mut record = cached.record.clone();
                    record.node_id = node_id;
                    return record;
                }
            }
        }

        let record = extract(node_id, path);
        if let Ok(mut entries) = self.entries.lock() {
            entries.insert(
                path.to_path_buf(),
                CachedRecord {
                    modified,
                    record: record.clone(),
                },
            );
        }

        record
    }

    /// Extracts metadata for many files in parallel.
    ///
    /// The result preserves the input order, which keeps CLI and GUI output
    /// deterministic (principle V).
    #[must_use]
    pub fn extract_many(&self, files: &[(NodeId, PathBuf)]) -> Vec<MetadataRecord> {
        use rayon::prelude::*;

        files
            .par_iter()
            .map(|(node_id, path)| self.extract(*node_id, path))
            .collect()
    }
}

/// A keyed view over extracted records.
#[derive(Debug, Default, Clone)]
pub struct MetadataIndex {
    records: HashMap<NodeId, MetadataRecord>,
}

impl MetadataIndex {
    /// Creates an empty index.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts or replaces a record.
    pub fn insert(&mut self, record: MetadataRecord) {
        self.records.insert(record.node_id, record);
    }

    /// Looks a record up by node.
    #[must_use]
    pub fn get(&self, node_id: NodeId) -> Option<&MetadataRecord> {
        self.records.get(&node_id)
    }

    /// Number of records.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns `true` when the index is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Returns records ordered by descending size of the corresponding node.
    #[must_use]
    pub fn sorted_by_size(&self, tree: &FsTree) -> Vec<&MetadataRecord> {
        let mut records: Vec<&MetadataRecord> = self.records.values().collect();
        records.sort_by_key(|record| std::cmp::Reverse(tree.node(record.node_id).size.logical));
        records
    }
}

/// Collects files that have a supported metadata family.
///
/// Only regular, accessible files are returned; links and denied objects are
/// skipped because their metadata cannot be trusted (principle II).
#[must_use]
pub fn supported_files(tree: &FsTree) -> Vec<(NodeId, PathBuf)> {
    let mut files = Vec::new();
    for index in 0..tree.len() {
        let Ok(node_id) = NodeId::try_from(index) else {
            continue;
        };
        let node = tree.node(node_id);
        if node.is_removed()
            || node.kind != NodeKind::File
            || !node.is_accessible()
            || MetadataKind::for_path(&tree.path(node_id)).is_none()
        {
            continue;
        }
        files.push((node_id, tree.path(node_id)));
    }

    files
}

/// Parses a `WIDTHxHEIGHT` resolution written as `3840x2160`.
///
/// # Errors
/// Returns [`freespace_core::Error::InvalidArgument`] when the value is
/// malformed.
pub fn parse_resolution(value: &str) -> Result<(u32, u32)> {
    let (width, height) = value.split_once(['x', 'X', '*']).ok_or_else(|| {
        freespace_core::Error::InvalidArgument(format!(
            "invalid resolution `{value}`; expected `WIDTHxHEIGHT`"
        ))
    })?;
    let width = width.trim().parse::<u32>().map_err(|_| {
        freespace_core::Error::InvalidArgument(format!("invalid resolution width in `{value}`"))
    })?;
    let height = height.trim().parse::<u32>().map_err(|_| {
        freespace_core::Error::InvalidArgument(format!("invalid resolution height in `{value}`"))
    })?;
    if width == 0 || height == 0 {
        return Err(freespace_core::Error::InvalidArgument(format!(
            "resolution `{value}` must be positive"
        )));
    }

    Ok((width, height))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        MetadataCache, MetadataFilter, MetadataKind, MetadataRecord, MetadataStatus,
        parse_resolution, tags,
    };

    #[test]
    fn classifies_extensions() {
        assert_eq!(
            MetadataKind::for_path(Path::new("photo.JPG")),
            Some(MetadataKind::Exif)
        );
        assert_eq!(
            MetadataKind::for_path(Path::new("song.flac")),
            Some(MetadataKind::Audio)
        );
        assert_eq!(
            MetadataKind::for_path(Path::new("clip.MP4")),
            Some(MetadataKind::Video)
        );
        assert_eq!(
            MetadataKind::for_path(Path::new("tool.exe")),
            Some(MetadataKind::Pe)
        );
        assert_eq!(MetadataKind::for_path(Path::new("notes.txt")), None);
    }

    #[test]
    fn applies_metadata_filters() {
        let mut record = MetadataRecord::new(1, MetadataKind::Video, MetadataStatus::Ok);
        record.insert(tags::WIDTH, "3840");
        record.insert(tags::HEIGHT, "2160");
        record.insert(tags::DURATION_SECS, "120");
        record.insert(tags::CODEC, "h264");

        let filter = MetadataFilter {
            min_width: Some(1920),
            min_height: Some(1080),
            min_duration_secs: Some(60),
            codec: Some("H264".to_owned()),
            ..MetadataFilter::default()
        };
        assert!(filter.matches(&record));

        assert!(
            !MetadataFilter {
                min_width: Some(7680),
                ..MetadataFilter::default()
            }
            .matches(&record)
        );
        assert!(
            !MetadataFilter {
                kinds: vec![MetadataKind::Audio],
                ..MetadataFilter::default()
            }
            .matches(&record)
        );

        let mut broken = record.clone();
        broken.status = MetadataStatus::Corrupted;
        assert!(!MetadataFilter::default().matches(&broken));
    }

    #[test]
    fn caches_and_invalidates_by_modification_time() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("sample.txt");
        std::fs::write(&path, b"not metadata").expect("write");
        let cache = MetadataCache::new();

        let first = cache.extract(7, &path);
        assert_eq!(first.status, MetadataStatus::Unsupported);
        assert_eq!(cache.len(), 1);

        let second = cache.extract(9, &path);
        assert_eq!(second.node_id, 9);
        assert_eq!(second.status, MetadataStatus::Unsupported);
    }

    #[test]
    fn parses_resolutions() {
        assert_eq!(
            parse_resolution("3840x2160").expect("resolution"),
            (3840, 2160)
        );
        assert!(parse_resolution("4K").is_err());
        assert!(parse_resolution("0x10").is_err());
    }
}
