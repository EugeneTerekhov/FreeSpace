//! `freespace archives` — list archive contents without unpacking (FR-066…FR-069).

use std::path::Path;

use freespace_analysis::{ArchiveInspection, ArchiveLimits, find_archives, inspect};
use freespace_core::{
    Result,
    size::{Units, format_bytes},
};
use freespace_platform::paths;
use serde_json::{Value, json};

use super::Outcome;

/// Maximum number of archives inspected in one directory run.
pub const MAX_ARCHIVES: usize = 1_000;

/// Runs `freespace archives`.
///
/// # Errors
/// Returns an error when a single archive cannot be read or when the export
/// cannot be written.
pub fn run(path: &Path, export: Option<&Path>, units: Units) -> Result<Outcome> {
    let limits = ArchiveLimits::default();
    let mut warnings = Vec::new();
    let mut inspections = Vec::new();

    if path.is_dir() {
        let archives = find_archives(path, MAX_ARCHIVES);
        if archives.is_empty() {
            return Ok(Outcome::new(
                json!({ "items": [], "count": 0 }),
                "no archives found\n".to_owned(),
            ));
        }
        for archive in archives {
            match inspect(&archive, limits) {
                Ok(inspection) => inspections.push(inspection),
                Err(error) => warnings.push(format!("{}: {error}", paths::display(&archive))),
            }
        }
    } else {
        inspections.push(inspect(path, limits)?);
    }

    if let Some(export) = export {
        freespace_report::archive_list::write(&inspections, export)?;
    }

    let items: Vec<Value> = inspections.iter().map(inspection_json).collect();
    let mut human = format!("{} archives\n", inspections.len());
    for inspection in &inspections {
        human.push_str(&format!(
            "{}  {}  {} entries  {}\n",
            paths::display(&inspection.archive),
            inspection.format.as_str(),
            inspection.entries.len(),
            format_bytes(inspection.total_size, units)
        ));
        for entry in inspection.largest(10) {
            human.push_str(&format!(
                "  {:>12}  {}\n",
                format_bytes(entry.size, units),
                entry.internal_path
            ));
        }
    }

    Ok(Outcome::new(
        json!({
            "items": items,
            "count": inspections.len(),
            "export": export.map(paths::display),
        }),
        human,
    )
    .with_warnings(warnings))
}

fn inspection_json(inspection: &ArchiveInspection) -> Value {
    json!({
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
    })
}

#[cfg(test)]
mod tests {
    use std::{io::Write as _, path::PathBuf};

    use freespace_core::size::Units;
    use zip::write::SimpleFileOptions;

    use super::run;

    fn archive() -> (tempfile::TempDir, PathBuf) {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("sample.zip");
        let file = std::fs::File::create(&path).expect("create");
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("docs/readme.txt", SimpleFileOptions::default())
            .expect("entry");
        writer.write_all(b"hello").expect("write");
        writer
            .start_file("payload.bin", SimpleFileOptions::default())
            .expect("entry");
        writer.write_all(&[0u8; 128]).expect("write");
        writer.finish().expect("finish");

        (directory, path)
    }

    #[test]
    fn lists_archive_contents_and_largest_entries() {
        let (_directory, path) = archive();
        let outcome = run(&path, None, Units::Binary).expect("archives");
        let items = outcome.data["items"].as_array().expect("items");
        let entries = items[0]["entries"].as_array().expect("entries");

        assert_eq!(outcome.data["count"].as_u64(), Some(1));
        assert_eq!(items[0]["format"], "zip");
        assert_eq!(entries.len(), 2);
        assert_eq!(items[0]["total_size"].as_u64(), Some(133));
        assert!(outcome.human.contains("payload.bin"));
    }

    #[test]
    fn exports_csv_json_xml_and_html() {
        let (_directory, path) = archive();
        let inspection =
            freespace_analysis::inspect(&path, freespace_analysis::ArchiveLimits::default())
                .expect("inspect");
        let output = tempfile::tempdir().expect("output");

        for extension in ["csv", "json", "xml", "html"] {
            let path = output.path().join(format!("list.{extension}"));
            freespace_report::archive_list::write(std::slice::from_ref(&inspection), &path)
                .expect("export");
            let text = std::fs::read_to_string(&path).expect("read");
            assert!(text.contains("payload.bin"), "{extension} content");
        }
    }
}
