//! Archive content-list export shared by the CLI and the GUI.
//!
//! The four text formats keep the same columns and escaping rules used by the
//! main report writers; XLSX is not produced for archive lists because the
//! contract only requires an export of the listing.

use std::{fmt::Write as _, path::Path};

use freespace_analysis::ArchiveInspection;
use freespace_core::{Error, Result};
use freespace_platform::paths;
use serde_json::json;

/// Writes the archive content list as CSV, JSON, XML or HTML.
///
/// # Errors
/// Returns [`Error::Unsupported`] for other extensions and an error when the
/// file cannot be written.
pub fn write(inspections: &[ArchiveInspection], path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("json")
        .to_ascii_lowercase();

    match extension.as_str() {
        "csv" => write_csv(inspections, path),
        "json" => write_json(inspections, path),
        "xml" => write_xml(inspections, path),
        "html" | "htm" => write_html(inspections, path),
        other => Err(Error::Unsupported(format!(
            "archive list export to `.{other}` is not supported; use csv, json, xml or html"
        ))),
    }
}

const CSV_HEADER: &str =
    "archive,format,internal_path,size,compressed_size,is_directory,encrypted,share";

fn write_csv(inspections: &[ArchiveInspection], path: &Path) -> Result<()> {
    let mut text = String::from(CSV_HEADER);
    text.push('\n');
    for inspection in inspections {
        for entry in &inspection.entries {
            let _ = writeln!(
                text,
                "\"{}\",\"{}\",\"{}\",{},{},{},{},{:.6}",
                csv_field(&paths::display(&inspection.archive)),
                inspection.format.as_str(),
                csv_field(&entry.internal_path),
                entry.size,
                entry
                    .compressed_size
                    .map_or_else(String::new, |value| value.to_string()),
                entry.is_directory,
                entry.encrypted,
                entry.share,
            );
        }
    }

    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(text.as_bytes());
    std::fs::write(path, bytes)?;

    Ok(())
}

fn csv_field(value: &str) -> String {
    value.replace('"', "\"\"")
}

fn write_json(inspections: &[ArchiveInspection], path: &Path) -> Result<()> {
    let value = json!({
        "items": inspections.iter().map(|inspection| json!({
            "archive": paths::display(&inspection.archive),
            "format": inspection.format.as_str(),
            "total_size": inspection.total_size,
            "encrypted_count": inspection.encrypted_count,
            "file_count": inspection.file_count(),
            "entries": inspection.entries.iter().map(|entry| json!({
                "path": entry.internal_path,
                "size": entry.size,
                "compressed_size": entry.compressed_size,
                "is_directory": entry.is_directory,
                "encrypted": entry.encrypted,
                "share": entry.share,
            })).collect::<Vec<_>>(),
            "warnings": inspection.warnings,
        })).collect::<Vec<_>>(),
    });
    let text = serde_json::to_string_pretty(&value)
        .map_err(|error| Error::Other(format!("cannot encode archive list: {error}")))?;
    std::fs::write(path, text)?;

    Ok(())
}

fn write_xml(inspections: &[ArchiveInspection], path: &Path) -> Result<()> {
    let mut text = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<archives>\n");
    for inspection in inspections {
        let _ = writeln!(
            text,
            "  <archive path=\"{}\" format=\"{}\" total-size=\"{}\">",
            escape_xml(&paths::display(&inspection.archive)),
            inspection.format.as_str(),
            inspection.total_size
        );
        for entry in &inspection.entries {
            let _ = writeln!(
                text,
                "    <entry path=\"{}\" size=\"{}\" compressed-size=\"{}\" directory=\"{}\" encrypted=\"{}\" share=\"{:.6}\"/>",
                escape_xml(&entry.internal_path),
                entry.size,
                entry.compressed_size.unwrap_or_default(),
                entry.is_directory,
                entry.encrypted,
                entry.share
            );
        }
        text.push_str("  </archive>\n");
    }
    text.push_str("</archives>\n");
    std::fs::write(path, text)?;

    Ok(())
}

fn write_html(inspections: &[ArchiveInspection], path: &Path) -> Result<()> {
    let mut text = String::from(
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <title>FreeSpace archive contents</title>\
         <style>body{font-family:Segoe UI,Arial,sans-serif;margin:24px}\
         table{border-collapse:collapse;margin:12px 0}th,td{border:1px solid #ccc;padding:4px 8px}\
         th{background:#f2f2f2}</style></head><body><h1>Archive contents</h1>",
    );
    for inspection in inspections {
        let _ = writeln!(
            text,
            "<section><h2>{} <small>({})</small></h2><table><thead><tr>\
             <th>Path</th><th>Size</th><th>Compressed</th><th>Directory</th><th>Encrypted</th>\
             <th>Share</th></tr></thead><tbody>",
            escape_html(&paths::display(&inspection.archive)),
            inspection.format.as_str()
        );
        for entry in &inspection.entries {
            let _ = writeln!(
                text,
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{:.2}%</td></tr>",
                escape_html(&entry.internal_path),
                entry.size,
                entry
                    .compressed_size
                    .map_or_else(String::new, |value| value.to_string()),
                entry.is_directory,
                entry.encrypted,
                entry.share * 100.0
            );
        }
        text.push_str("</tbody></table></section>");
    }
    text.push_str("</body></html>");
    std::fs::write(path, text)?;

    Ok(())
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn escape_html(value: &str) -> String {
    crate::html::escape_html(value)
}

#[cfg(test)]
mod tests {
    use freespace_analysis::{ArchiveEntry, ArchiveFormat, ArchiveInspection};

    use super::write;

    fn inspection() -> ArchiveInspection {
        ArchiveInspection {
            archive: "sample.zip".into(),
            format: ArchiveFormat::Zip,
            entries: vec![ArchiveEntry::file("payload.bin", 128, Some(64))],
            total_size: 128,
            encrypted_count: 0,
            warnings: Vec::new(),
        }
    }

    #[test]
    fn writes_every_text_format() {
        let directory = tempfile::tempdir().expect("temp dir");

        for extension in ["csv", "json", "xml", "html"] {
            let path = directory.path().join(format!("list.{extension}"));
            write(std::slice::from_ref(&inspection()), &path).expect("write");
            let text = std::fs::read_to_string(&path).expect("read");
            assert!(text.contains("payload.bin"), "{extension} content");
        }

        assert!(
            write(
                std::slice::from_ref(&inspection()),
                &directory.path().join("list.xlsx")
            )
            .is_err()
        );
    }
}
