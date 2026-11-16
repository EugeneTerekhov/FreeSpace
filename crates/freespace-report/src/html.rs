//! Self-contained HTML report writer with mandatory escaping.

use std::{
    fs::File,
    io::{BufWriter, Write as _},
    path::Path,
};

use freespace_core::{Error, Result};

use crate::report::{Cell, Report, SectionName};

/// Writes a standalone HTML report with inline styles and no external assets.
///
/// # Errors
/// Returns an error when the output file cannot be created or written.
pub fn write(report: &Report<'_>, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let file = File::create(path)?;
    let mut out = BufWriter::new(file);

    write_header(&mut out, report).map_err(io_error)?;
    write_summary(&mut out, report).map_err(io_error)?;

    for section in report.sections() {
        write!(
            out,
            "<section><h2>{}</h2><table><thead><tr>",
            escape_html(section.as_str())
        )
        .map_err(io_error)?;
        for header in section.headers() {
            write!(out, "<th>{}</th>", escape_html(header)).map_err(io_error)?;
        }
        write!(out, "</tr></thead><tbody>").map_err(io_error)?;

        if section == SectionName::Tree {
            for row in report.html_tree_rows() {
                write_html_row(&mut out, &row.cells()).map_err(io_error)?;
            }
        } else {
            report
                .for_each_row(section, |row| write_html_row(&mut out, &row))
                .map_err(io_error)?;
        }

        write!(out, "</tbody></table></section>").map_err(io_error)?;
    }

    write!(
        out,
        "<footer>FreeSpace {}</footer></body></html>",
        escape_html(&report.meta.app_version)
    )
    .map_err(io_error)?;
    out.flush().map_err(io_error)?;

    Ok(())
}

fn write_header(out: &mut BufWriter<File>, report: &Report<'_>) -> std::io::Result<()> {
    let title = format!(
        "FreeSpace report — {}",
        report.meta.generated_at.format_iso8601()
    );
    write!(
        out,
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>{}</title>\
         <style>body{{font-family:Segoe UI,Arial,sans-serif;margin:24px;color:#1b1b1b}}\
         table{{border-collapse:collapse;margin:12px 0;font-size:13px}}\
         th,td{{border:1px solid #d0d0d0;padding:4px 8px;text-align:left;vertical-align:top}}\
         th{{background:#f2f2f2}}tbody tr:nth-child(even){{background:#fafafa}}\
         h1{{font-size:24px}}h2{{font-size:18px;margin-top:28px}}\
         .meta{{display:grid;grid-template-columns:max-content 1fr;gap:4px 16px;font-size:14px}}\
         footer{{margin-top:24px;color:#666;font-size:12px}}</style></head><body>",
        escape_html(&title)
    )?;
    write!(out, "<h1>{}</h1>", escape_html(&title))?;

    Ok(())
}

fn write_summary(out: &mut BufWriter<File>, report: &Report<'_>) -> std::io::Result<()> {
    write!(out, "<section><h2>Summary</h2><div class=\"meta\">")?;
    write!(
        out,
        "<span>Report version</span><span>{}</span>",
        report.meta.report_version
    )?;
    write!(
        out,
        "<span>Generated</span><span>{}</span>",
        escape_html(&report.meta.generated_at.format_iso8601())
    )?;
    write!(
        out,
        "<span>Application</span><span>{}</span>",
        escape_html(&report.meta.app_version)
    )?;
    write!(
        out,
        "<span>Units</span><span>{}</span>",
        escape_html(&report.meta.units)
    )?;
    write!(
        out,
        "<span>Session</span><span>{}</span>",
        escape_html(&report.meta.session_id)
    )?;
    if let Some(volume) = &report.meta.volume {
        write!(
            out,
            "<span>Volume</span><span>{} ({}, {})</span>",
            escape_html(&volume.letter),
            escape_html(&volume.label),
            escape_html(&volume.filesystem)
        )?;
    }
    for root in &report.meta.roots {
        write!(out, "<span>Root</span><span>{}</span>", escape_html(root))?;
    }
    write!(
        out,
        "<span>Totals</span><span>{} logical bytes, {} on disk, {} physical</span>",
        report.totals.logical, report.totals.on_disk, report.totals.physical
    )?;
    write!(out, "</div></section>")?;

    Ok(())
}

fn write_html_row<W: std::io::Write>(out: &mut W, cells: &[Cell]) -> std::io::Result<()> {
    write!(out, "<tr>")?;
    for cell in cells {
        let value = match cell {
            Cell::Text(text) => escape_html(text),
            other => escape_html(&other.to_text()),
        };
        write!(out, "<td>{value}</td>")?;
    }
    write!(out, "</tr>")
}

/// Escapes text for HTML content and attribute values.
#[must_use]
pub fn escape_html(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            other => escaped.push(other),
        }
    }

    escaped
}

fn io_error(error: std::io::Error) -> Error {
    Error::Io(error)
}

#[cfg(test)]
mod tests {
    use freespace_core::{Timestamp, size::SizeMetrics};

    use crate::report::{Report, ReportMeta};

    use super::{escape_html, write};

    #[test]
    fn escapes_dangerous_text() {
        assert_eq!(
            escape_html("<script>alert('x') & \"y\"</script>"),
            "&lt;script&gt;alert(&#39;x&#39;) &amp; &quot;y&quot;&lt;/script&gt;"
        );
    }

    #[test]
    fn writes_a_self_contained_document() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("report.html");
        let report = Report::new(
            ReportMeta::new(Timestamp::UNIX_EPOCH, "1.0.0").with_session("session"),
            SizeMetrics::ZERO,
        );

        write(&report, &path).expect("write");
        let text = std::fs::read_to_string(&path).expect("read");

        assert!(text.starts_with("<!DOCTYPE html>"));
        assert!(text.contains("<style>"));
        assert!(text.contains("Summary"));
        assert!(!text.contains("http://"));
    }
}
