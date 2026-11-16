//! XLSX report writer through `rust_xlsxwriter`.
//!
//! Sheets follow `contracts/report-formats.md`: `Summary`, `Tree`, `TopFiles`,
//! `Types`, `Duplicates` and `Owners`. Large sections use constant-memory
//! worksheets so an export of millions of nodes does not exhaust RAM.

use std::path::Path;

use freespace_core::{Error, Result};
use rust_xlsxwriter::{Format, Workbook, Worksheet, XlsxError};

use crate::report::{Cell, Report};

/// Writes a report as an XLSX workbook.
///
/// # Errors
/// Returns an error when the workbook cannot be written.
pub fn write(report: &Report<'_>, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let mut workbook = Workbook::new();
    write_summary(&mut workbook, report)?;

    let header_format = Format::new().set_bold();
    let size_format = Format::new().set_num_format("#,##0");
    let share_format = Format::new().set_num_format("0.00%");

    for section in report.sections() {
        let worksheet = workbook
            .add_worksheet_with_constant_memory()
            .map_err(xlsx_error)?;
        worksheet
            .set_name(section.sheet_name())
            .map_err(xlsx_error)?;
        worksheet.set_freeze_panes(1, 0).map_err(xlsx_error)?;

        for (column, header) in section.headers().iter().enumerate() {
            worksheet
                .write_string_with_format(0, column as u16, *header, &header_format)
                .map_err(xlsx_error)?;
            let width = match *header {
                "path" | "internal_path" => 60.0,
                "attributes" => 24.0,
                "hash" => 66.0,
                "group_id" => 38.0,
                _ => 16.0,
            };
            worksheet
                .set_column_width(column as u16, width)
                .map_err(xlsx_error)?;
        }

        let mut row_index = 1u32;
        let mut last_column = 0usize;
        report.for_each_row(section, |row| {
            last_column = row.len();
            for (column, cell) in row.iter().enumerate() {
                write_cell(
                    worksheet,
                    row_index,
                    column as u16,
                    cell,
                    &size_format,
                    &share_format,
                )?;
            }
            row_index = row_index.saturating_add(1);
            Ok::<(), Error>(())
        })?;

        if last_column > 0 {
            worksheet
                .autofilter(0, 0, row_index.saturating_sub(1), (last_column - 1) as u16)
                .map_err(xlsx_error)?;
        }
    }

    let summary = workbook.worksheet_from_index(0).map_err(xlsx_error)?;
    summary.set_column_width(0, 24.0).map_err(xlsx_error)?;
    summary.set_column_width(1, 80.0).map_err(xlsx_error)?;

    workbook.save(path).map_err(xlsx_error)?;

    Ok(())
}

fn write_summary(workbook: &mut Workbook, report: &Report<'_>) -> Result<()> {
    let worksheet = workbook.add_worksheet();
    worksheet.set_name("Summary").map_err(xlsx_error)?;
    let title = Format::new().set_bold().set_font_size(14);
    let bold = Format::new().set_bold();

    worksheet
        .write_string_with_format(0, 0, "FreeSpace report", &title)
        .map_err(xlsx_error)?;

    let rows = [
        ("Report version", report.meta.report_version.to_string()),
        ("Generated", report.meta.generated_at.format_iso8601()),
        ("Application", report.meta.app_version.clone()),
        ("Units", report.meta.units.clone()),
        ("Session", report.meta.session_id.clone()),
        (
            "Volume",
            report
                .meta
                .volume
                .as_ref()
                .map(|volume| {
                    format!(
                        "{} ({}, {}, {})",
                        volume.letter, volume.label, volume.filesystem, volume.scan_method
                    )
                })
                .unwrap_or_default(),
        ),
        ("Roots", report.meta.roots.join("; ")),
        (
            "Sections",
            report
                .sections()
                .iter()
                .map(|section| section.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        ),
        ("Rows", report.total_rows().to_string()),
        ("Logical size", report.totals.logical.to_string()),
        ("On-disk size", report.totals.on_disk.to_string()),
        ("Physical size", report.totals.physical.to_string()),
        ("ADS size", report.totals.ads.to_string()),
    ];

    for (index, (label, value)) in rows.iter().enumerate() {
        let row = u32::try_from(index + 1).unwrap_or(u32::MAX);
        worksheet
            .write_string_with_format(row, 0, *label, &bold)
            .map_err(xlsx_error)?;
        worksheet.write_string(row, 1, value).map_err(xlsx_error)?;
    }

    Ok(())
}

fn write_cell(
    worksheet: &mut Worksheet,
    row: u32,
    column: u16,
    cell: &Cell,
    size_format: &Format,
    share_format: &Format,
) -> Result<()> {
    match cell {
        Cell::Text(value) => {
            worksheet
                .write_string(row, column, value)
                .map_err(xlsx_error)?;
        }
        Cell::Unsigned(value) => {
            worksheet
                .write_number_with_format(row, column, *value as f64, size_format)
                .map_err(xlsx_error)?;
        }
        Cell::Signed(value) => {
            worksheet
                .write_number_with_format(row, column, *value as f64, size_format)
                .map_err(xlsx_error)?;
        }
        Cell::Float(value) => {
            worksheet
                .write_number_with_format(row, column, *value, share_format)
                .map_err(xlsx_error)?;
        }
        Cell::Bool(value) => {
            worksheet
                .write_boolean(row, column, *value)
                .map_err(xlsx_error)?;
        }
    }

    Ok(())
}

fn xlsx_error(error: XlsxError) -> Error {
    Error::Operation(format!("cannot write XLSX report: {error}"))
}

#[cfg(test)]
mod tests {
    use freespace_core::{Timestamp, size::SizeMetrics};

    use crate::report::{Report, ReportMeta};

    use super::write;

    #[test]
    fn writes_a_workbook_with_a_summary_sheet() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("report.xlsx");
        let report = Report::new(
            ReportMeta::new(Timestamp::UNIX_EPOCH, "1.0.0").with_session("session"),
            SizeMetrics::ZERO,
        );

        write(&report, &path).expect("write");
        let bytes = std::fs::read(&path).expect("read");

        assert!(bytes.starts_with(b"PK"), "XLSX is a ZIP container");
        assert!(bytes.len() > 1024);
    }
}
