//! Streaming CSV report writer (contract `contracts/report-formats.md`).

use std::{
    fs::File,
    io::{BufWriter, Write as _},
    path::Path,
};

use freespace_core::{Error, Result};

use crate::report::Report;

/// UTF-8 byte-order mark required for Excel compatibility.
const BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// Writes a report as UTF-8 CSV with a BOM, one block per section.
///
/// # Errors
/// Returns an error when the output file cannot be created or written.
pub fn write(report: &Report<'_>, path: &Path) -> Result<()> {
    ensure_parent(path)?;
    let file = File::create(path)?;
    let mut output = BufWriter::new(file);
    output.write_all(&BOM)?;

    let mut writer = csv::WriterBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_writer(output);

    let mut first = true;
    for section in report.sections() {
        if !first {
            writer
                .write_byte_record(&csv::ByteRecord::new())
                .map_err(|error| Error::Operation(format!("cannot write CSV: {error}")))?;
        }
        first = false;

        writer
            .write_record([format!("# section: {}", section.as_str())])
            .map_err(|error| Error::Operation(format!("cannot write CSV: {error}")))?;
        writer
            .write_record(section.headers())
            .map_err(|error| Error::Operation(format!("cannot write CSV: {error}")))?;

        report.for_each_row(section, |row| {
            let record: Vec<String> = row.iter().map(crate::report::Cell::to_text).collect();
            writer
                .write_record(record)
                .map(|_| ())
                .map_err(|error| Error::Operation(format!("cannot write CSV: {error}")))
        })?;
    }

    writer
        .flush()
        .map_err(|error| Error::Operation(format!("cannot write CSV: {error}")))?;

    Ok(())
}

fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use freespace_core::{Timestamp, size::SizeMetrics};

    use crate::report::{Report, ReportMeta};

    use super::write;

    #[test]
    fn writes_bom_headers_and_totals() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("report.csv");
        let report = Report::new(ReportMeta::default(), SizeMetrics::ZERO);

        write(&report, &path).expect("write");

        let bytes = std::fs::read(&path).expect("read");
        assert!(bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
        let text = String::from_utf8(bytes[3..].to_vec()).expect("utf-8");
        assert!(!text.contains("section"), "no data means no sections");
        assert_eq!(Timestamp::UNIX_EPOCH.as_millis(), 0);
    }
}
