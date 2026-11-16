#![forbid(unsafe_code)]

//! FreeSpace report and snapshot formats.
//!
//! * [`snapshot`] — the versioned `.fssnap` container (CBOR + zstd + SHA-256);
//! * [`diff`] — comparison of two snapshots;
//! * [`report`] — the shared report model and section schemas;
//! * [`csv`], [`xml`], [`html`], [`xlsx`] — streaming report writers;
//! * [`owners`] — the per-owner report section.

pub mod archive_list;
pub mod csv;
pub mod diff;
pub mod html;
pub mod owners;
pub mod report;
pub mod snapshot;
pub mod xlsx;
pub mod xml;

use std::path::Path;

pub use diff::{DiffChange, DiffEntry, GrowthEntry, SnapshotDiff, compare};
pub use owners::OwnerReport;
pub use report::{
    Cell, DuplicateRow, OwnerRow, Report, ReportMeta, ReportVolume, SectionName, TopRow, TreeRow,
    TypeRow, attributes_text,
};
pub use snapshot::{
    CONTAINER_VERSION, ColumnarNodes, FORMAT_VERSION, MAGIC, SnapshotPayload, SnapshotSettings,
    SnapshotVolume, from_tree,
};

/// Output format of an exported report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReportFormat {
    /// UTF-8 CSV with a BOM.
    Csv,
    /// XML document.
    Xml,
    /// Self-contained HTML.
    Html,
    /// Excel workbook.
    Xlsx,
}

impl ReportFormat {
    /// Every supported format.
    pub const ALL: [Self; 4] = [Self::Csv, Self::Xml, Self::Html, Self::Xlsx];

    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Xml => "xml",
            Self::Html => "html",
            Self::Xlsx => "xlsx",
        }
    }

    /// File extension without the dot.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        self.as_str()
    }

    /// Parses the command-line value.
    #[must_use]
    pub fn parse_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "csv" => Some(Self::Csv),
            "xml" => Some(Self::Xml),
            "html" | "htm" => Some(Self::Html),
            "xlsx" | "excel" => Some(Self::Xlsx),
            _ => None,
        }
    }
}

/// Writes `report` in the requested format.
///
/// # Errors
/// Returns an error when the output file cannot be written or the format is
/// not supported.
pub fn write_report(
    report: &Report<'_>,
    format: ReportFormat,
    path: &Path,
) -> freespace_core::Result<()> {
    match format {
        ReportFormat::Csv => csv::write(report, path),
        ReportFormat::Xml => xml::write(report, path),
        ReportFormat::Html => html::write(report, path),
        ReportFormat::Xlsx => xlsx::write(report, path),
    }
}
