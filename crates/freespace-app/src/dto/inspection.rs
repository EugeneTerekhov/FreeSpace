//! Inspection and report command payloads (US5).
//!
//! The shapes follow `specs/002-native-ui-redesign/contracts/ipc.md` §8. The
//! interface never recomputes metadata, owner totals or archive shares: every
//! value here is produced by `freespace-analysis` and copied unchanged.
//!
//! Types are re-exported from [`crate::dto`], so callers keep using
//! `freespace_app::dto::MetadataView`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::TaskState;

/// Maximum number of metadata tags returned for one file.
pub const MAX_METADATA_TAGS: usize = 256;

/// Maximum length of one metadata tag value, in characters.
pub const MAX_METADATA_TAG_CHARS: usize = 2_048;

/// Maximum PE image size inspected for version resources.
///
/// The PE reader loads the whole image; media and EXIF readers only parse
/// headers, so the bound applies to executables only.
pub const MAX_PE_IMAGE_BYTES: u64 = 256 * 1024 * 1024;

/// One metadata tag with the reader that produced it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataTag {
    /// Stable tag name from `freespace_analysis::metadata::tags`.
    pub key: String,
    /// Already rendered value.
    pub value: String,
    /// Localisable source key, for example `metadata.source.exif`.
    pub source: String,
}

/// Extracted metadata of one file (`metadata_get`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataView {
    /// Node inside the generation.
    pub node_id: u32,
    /// Full path.
    pub path: String,
    /// Metadata family: `exif`, `audio`, `video` or `pe`.
    pub kind: String,
    /// Localisable source key of the reader.
    pub source_key: String,
    /// Extraction status; `ok` for a successful read.
    pub status: String,
    /// True when the record can be displayed.
    pub available: bool,
    /// Extracted tags, sorted by key.
    pub tags: Vec<MetadataTag>,
    /// True when tags or values were truncated to the documented limits.
    pub truncated: bool,
    /// Logical size of the inspected file.
    pub size_bytes: u64,
    /// Modification time in Unix milliseconds, when known.
    pub modified_at: Option<i64>,
}

/// One owner row of `owners_page`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OwnerSummaryView {
    /// Dense owner index; `None` for the documented `unknown` category.
    pub owner_id: Option<u32>,
    /// Security identifier; empty for the unknown category.
    pub sid: String,
    /// Account name, or `unknown`.
    pub display_name: String,
    /// Total logical size of the owner's files.
    pub total_bytes: u64,
    /// Number of files owned by the account.
    pub file_count: u64,
    /// Share of the analysed bytes in `0.0..=1.0`.
    pub share: f64,
    /// False for the unknown category, which marks unreadable ownership.
    pub accessible: bool,
    /// Localisable label key; `owners.label.unknown` for the unknown category.
    pub label_key: String,
}

/// One page of owner summaries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OwnerSummaryPage {
    /// Generation the page belongs to.
    pub generation: u64,
    /// Cursor that produced this page.
    pub cursor: Option<String>,
    /// Cursor for the next page; `None` at the end.
    pub next_cursor: Option<String>,
    /// Rows, largest owner first.
    pub rows: Vec<OwnerSummaryView>,
    /// Rough total row count.
    pub total_estimate: u64,
    /// Total logical bytes covered by the report.
    pub total_bytes: u64,
    /// Bytes whose owner could not be resolved.
    pub unknown_bytes: u64,
    /// Files whose security descriptor could not be read.
    pub error_count: u64,
    /// Non-fatal notes as localisable keys.
    pub warnings: Vec<String>,
    /// Always `false`: a foreign cursor is rejected with `E_STALE_CURSOR`.
    pub stale_cursor: bool,
}

/// One entry of an archive listing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchiveEntryView {
    /// Path inside the archive, using `/` as the separator.
    pub internal_path: String,
    /// Uncompressed size in bytes; zero for directories.
    pub size: u64,
    /// Compressed size when the container reports it.
    pub compressed_size: Option<u64>,
    /// Whether the entry is a directory.
    pub is_directory: bool,
    /// Whether the entry is encrypted.
    pub encrypted: bool,
    /// Share of the archive's uncompressed content in `0.0..=1.0`.
    pub share: f64,
    /// Nesting depth derived from the internal path.
    pub depth: u8,
    /// Lower-cased extension without the dot, when the entry has one.
    pub extension: Option<String>,
}

/// One page of archive entries (`archives_entries_page`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchiveEntryPage {
    /// Inspected archive.
    pub path: String,
    /// Detected container format.
    pub format: String,
    /// Cursor that produced this page.
    pub cursor: Option<String>,
    /// Cursor for the next page; `None` at the end.
    pub next_cursor: Option<String>,
    /// Entries of this page, largest first.
    pub entries: Vec<ArchiveEntryView>,
    /// Number of entries in the archive.
    pub total_entries: u64,
    /// Number of regular files in the archive.
    pub total_files: u64,
    /// Total uncompressed size.
    pub total_size: u64,
    /// Number of encrypted entries.
    pub encrypted_count: u64,
    /// Non-fatal notes.
    pub warnings: Vec<String>,
    /// Always `false`: a foreign cursor is rejected with `E_STALE_CURSOR`.
    pub stale_cursor: bool,
}

/// Output format of an exported report or archive list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    /// UTF-8 CSV.
    Csv,
    /// XML document.
    Xml,
    /// Self-contained HTML.
    Html,
    /// Excel workbook.
    Xlsx,
    /// JSON archive listing.
    Json,
}

impl ExportFormat {
    /// Every supported format.
    pub const ALL: [Self; 5] = [Self::Csv, Self::Xml, Self::Html, Self::Xlsx, Self::Json];

    /// Stable wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Xml => "xml",
            Self::Html => "html",
            Self::Xlsx => "xlsx",
            Self::Json => "json",
        }
    }

    /// File extension without the dot.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        self.as_str()
    }

    /// Parses a wire or command-line value.
    #[must_use]
    pub fn parse_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "csv" => Some(Self::Csv),
            "xml" => Some(Self::Xml),
            "html" | "htm" => Some(Self::Html),
            "xlsx" | "excel" => Some(Self::Xlsx),
            "json" => Some(Self::Json),
            _ => None,
        }
    }

    /// Formats accepted by `freespace-report` writers.
    #[must_use]
    pub const fn supports_report(self) -> bool {
        matches!(self, Self::Csv | Self::Xml | Self::Html | Self::Xlsx)
    }

    /// Formats accepted by `freespace_report::archive_list::write`.
    #[must_use]
    pub const fn supports_archive_list(self) -> bool {
        matches!(self, Self::Csv | Self::Xml | Self::Html | Self::Json)
    }
}

/// One section of an exported report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportSection {
    /// Hierarchical node list.
    Tree,
    /// Largest files.
    Top,
    /// Space by extension.
    Types,
    /// Duplicate groups.
    Duplicates,
    /// Space by owner.
    Owners,
}

impl ExportSection {
    /// Every section in document order.
    pub const ALL: [Self; 5] = [
        Self::Tree,
        Self::Top,
        Self::Types,
        Self::Duplicates,
        Self::Owners,
    ];

    /// Stable wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tree => "tree",
            Self::Top => "top",
            Self::Types => "types",
            Self::Duplicates => "duplicates",
            Self::Owners => "owners",
        }
    }

    /// Parses a wire value.
    #[must_use]
    pub fn parse_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "tree" => Some(Self::Tree),
            "top" => Some(Self::Top),
            "types" => Some(Self::Types),
            "duplicates" | "dups" => Some(Self::Duplicates),
            "owners" => Some(Self::Owners),
            _ => None,
        }
    }

    /// Localisable progress key of the export stage.
    #[must_use]
    pub const fn phase_key(self) -> &'static str {
        match self {
            Self::Tree => "export.phase.tree",
            Self::Top => "export.phase.top",
            Self::Types => "export.phase.types",
            Self::Duplicates => "export.phase.duplicates",
            Self::Owners => "export.phase.owners",
        }
    }
}

/// Scope of an export preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportScope {
    /// Generation the export is built from.
    pub generation: u64,
    /// Absolute output path chosen by the user.
    pub out: String,
}

/// Which sections an export contains and how it treats an existing file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExportComposition {
    /// Sections to include; empty is rejected.
    pub sections: Vec<ExportSection>,
    /// Cap on tree rows in every format.
    pub max_tree_rows: Option<usize>,
    /// Explicit consent to replace an existing output file.
    pub overwrite: bool,
}

impl Default for ExportComposition {
    fn default() -> Self {
        Self {
            sections: ExportSection::ALL.to_vec(),
            max_tree_rows: None,
            overwrite: false,
        }
    }
}

/// Input of `export_preview`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportPreviewRequest {
    /// Generation and output path.
    pub scope: ExportScope,
    /// Report format.
    pub format: ExportFormat,
    /// Sections and overwrite consent.
    #[serde(default)]
    pub composition: ExportComposition,
}

/// One-time preview of an export.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportPreview {
    /// One-time preview identifier.
    pub preview_id: String,
    /// One-time confirmation token bound to the preview.
    pub confirmation_token: String,
    /// Report format.
    pub format: ExportFormat,
    /// Absolute output path.
    pub out: String,
    /// Sections in document order.
    pub sections: Vec<ExportSection>,
    /// Rough row count; `None` when a section cannot be estimated cheaply.
    pub row_estimate: Option<u64>,
    /// Total logical bytes.
    pub total_bytes: u64,
    /// Total allocated bytes.
    pub on_disk_bytes: u64,
    /// Total physical bytes.
    pub physical_bytes: u64,
    /// True when the preview grants overwrite permission.
    pub overwrite: bool,
    /// Localisable warning keys shown before the export starts.
    pub warnings: Vec<String>,
    /// Expiry in Unix milliseconds.
    pub expires_at: i64,
}

/// Live state of one export task (`export_status`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportStatus {
    /// Background task identifier.
    pub task_id: String,
    /// Lifecycle state.
    pub state: TaskState,
    /// Completed stages.
    pub processed: u64,
    /// Total stages.
    pub total: u64,
    /// Localisable phase key.
    pub phase_key: String,
    /// Output path.
    pub out: Option<String>,
    /// Bytes written so far; set on completion.
    pub bytes_written: Option<u64>,
    /// Rows written; set on completion.
    pub rows_written: u64,
    /// False after the point of no return.
    pub cancelable: bool,
    /// Start time in Unix milliseconds.
    pub started_at: i64,
    /// Finish time in Unix milliseconds.
    pub finished_at: Option<i64>,
    /// Failure envelope; partial files are removed by the writer contract.
    pub error: Option<crate::error::UiError>,
}

/// Result of a synchronous export such as `archives_export_list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportResult {
    /// Written file.
    pub path: String,
    /// Format that was written.
    pub format: ExportFormat,
    /// Size of the written file in bytes.
    pub bytes_written: u64,
    /// Number of written rows.
    pub rows_written: u64,
    /// Sections included in the file.
    pub sections: Vec<ExportSection>,
    /// Non-fatal notes.
    pub warnings: Vec<String>,
}

/// One process that holds a file (`unlock_locks`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileLock {
    /// Process identifier.
    pub process_id: u32,
    /// Application name reported by the Restart Manager.
    pub process_name: String,
    /// Service short name, when the holder is a service.
    pub service_name: String,
    /// Application type: main-window, service, explorer, console, critical…
    pub application_type: String,
    /// Access type; the Restart Manager reports application-level holders, so
    /// this is `unknown` unless a future platform API can refine it.
    pub access_type: String,
    /// Localisable key explaining the access type.
    pub access_type_key: String,
    /// Whether the manager can ask the application to close.
    pub can_soft_close: bool,
    /// Whether a reboot is required to release the object.
    pub requires_reboot: bool,
    /// Critical holders must never be terminated.
    pub critical: bool,
    /// Number of handles; `None` when the platform cannot report it.
    pub handle_count: Option<u32>,
}

/// How a lock is released.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnlockMode {
    /// Ask applications to close the object gracefully.
    Soft,
    /// Terminate the confirmed processes.
    Force,
}

impl UnlockMode {
    /// Stable wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Soft => "soft",
            Self::Force => "force",
        }
    }

    /// Parses a wire value.
    #[must_use]
    pub fn parse_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "soft" => Some(Self::Soft),
            "force" => Some(Self::Force),
            _ => None,
        }
    }
}

/// Outcome of `unlock_release` and `unlock_schedule_reboot`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnlockResult {
    /// Operation identifier; the journal entry identifier when one was written.
    pub operation_id: String,
    /// Inspected object.
    pub path: String,
    /// Applied mode; force for a reboot registration.
    pub mode: UnlockMode,
    /// Applications asked to close the object.
    pub released: u32,
    /// Processes that were terminated.
    pub terminated: Vec<u32>,
    /// Whether the object is accessible afterwards.
    pub available: bool,
    /// Whether deletion was put into the reboot queue.
    pub scheduled_for_reboot: bool,
    /// Non-fatal notes.
    pub warnings: Vec<String>,
    /// Journal entry, when the local store was available.
    pub journal_entry_id: Option<String>,
}

/// Result of validating an output path before writing a report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathValidation {
    /// Checked path.
    pub path: String,
    /// True when the path can be written.
    pub valid: bool,
    /// Whether the file already exists.
    pub exists: bool,
    /// Whether the path points at an existing directory.
    pub is_directory: bool,
    /// Whether the parent directory already exists.
    pub parent_exists: bool,
    /// False when an existing file is read-only.
    pub writable: bool,
    /// Localisable key when the path is invalid.
    pub error_key: Option<String>,
}

/// Metadata labels used by the UI, keyed by the metadata family.
#[must_use]
pub fn metadata_source_key(kind: &str) -> String {
    format!("metadata.source.{kind}")
}

/// Tags sorted by key, truncated to [`MAX_METADATA_TAGS`] and
/// [`MAX_METADATA_TAG_CHARS`].
#[must_use]
pub fn bounded_metadata_tags(
    tags: &BTreeMap<String, String>,
    source: &str,
) -> (Vec<MetadataTag>, bool) {
    let mut truncated = tags.len() > MAX_METADATA_TAGS;
    let mut result = Vec::with_capacity(tags.len().min(MAX_METADATA_TAGS));

    for (key, value) in tags.iter().take(MAX_METADATA_TAGS) {
        let value = if value.chars().count() > MAX_METADATA_TAG_CHARS {
            truncated = true;
            value.chars().take(MAX_METADATA_TAG_CHARS).collect()
        } else {
            value.clone()
        };
        result.push(MetadataTag {
            key: key.clone(),
            value,
            source: source.to_owned(),
        });
    }

    (result, truncated)
}

/// Mixes a filter or owner scope into a generation so that a cursor produced
/// for another request is rejected with `E_STALE_CURSOR`.
///
/// The returned value is only used inside an opaque cursor; responses keep the
/// real generation.
#[must_use]
pub(crate) fn scoped_generation(generation: u64, scope: &str) -> u64 {
    // FNV-1a keeps the cursor stable across processes and platforms.
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in scope.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }

    let mixed = generation.rotate_left(17) ^ hash;
    if mixed == 0 { 1 } else { mixed }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        ExportComposition, ExportFormat, ExportSection, MAX_METADATA_TAGS, bounded_metadata_tags,
        scoped_generation,
    };

    #[test]
    fn parses_formats_and_separates_report_from_archive_formats() {
        assert_eq!(ExportFormat::parse_value("CSV"), Some(ExportFormat::Csv));
        assert_eq!(ExportFormat::parse_value("htm"), Some(ExportFormat::Html));
        assert_eq!(ExportFormat::parse_value("pdf"), None);
        assert!(ExportFormat::Xlsx.supports_report());
        assert!(!ExportFormat::Xlsx.supports_archive_list());
        assert!(ExportFormat::Json.supports_archive_list());
        assert!(!ExportFormat::Json.supports_report());
    }

    #[test]
    fn composition_defaults_to_every_section() {
        let composition = ExportComposition::default();

        assert_eq!(composition.sections.len(), 5);
        assert_eq!(composition.sections.first(), Some(&ExportSection::Tree));
        assert!(!composition.overwrite);
        assert!(composition.max_tree_rows.is_none());
        assert_eq!(
            ExportSection::parse_value("dups"),
            Some(ExportSection::Duplicates)
        );
        assert!(
            ExportSection::Owners
                .phase_key()
                .starts_with("export.phase.")
        );
    }

    #[test]
    fn bounds_metadata_tags() {
        let mut tags = BTreeMap::new();
        for index in 0..MAX_METADATA_TAGS + 2 {
            tags.insert(format!("Key{index:04}"), "value".to_owned());
        }
        tags.insert("Long".to_owned(), "x".repeat(5_000));

        let (bounded, truncated) = bounded_metadata_tags(&tags, "metadata.source.pe");

        assert!(truncated);
        assert!(bounded.len() <= MAX_METADATA_TAGS);
        assert!(bounded.iter().all(|tag| tag.source == "metadata.source.pe"));
        assert!(bounded.iter().all(|tag| tag.value.chars().count() <= 2_048));
    }

    #[test]
    fn scoped_generations_differ_for_other_requests() {
        let first = scoped_generation(7, "owners|text=a");
        let second = scoped_generation(7, "owners|text=b");
        let repeated = scoped_generation(7, "owners|text=a");

        assert_eq!(first, repeated);
        assert_ne!(first, second);
        assert_ne!(first, 0);
        assert_ne!(scoped_generation(0, ""), 0);
    }
}
