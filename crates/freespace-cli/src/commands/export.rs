//! `freespace export` — CSV, XML, HTML and XLSX reports (FR-070…FR-073).

use std::path::Path;

use freespace_analysis::{
    DuplicateOptions, annotate, find_duplicates, group_by_extension, top_files,
};
use freespace_core::{Error, Result, Timestamp, size::Units};
use freespace_platform::paths;
use freespace_report::{Report, ReportFormat, ReportMeta, ReportVolume, SectionName, write_report};
use serde_json::json;

use super::{
    Outcome,
    scan::{ScanArgs, execute},
};

/// Number of top files captured in an exported report.
const EXPORT_TOP_LIMIT: usize = 1_000;

/// Options of `freespace export`.
#[derive(Debug, Clone)]
pub struct ExportArgs<'a> {
    /// Volume or directory.
    pub path: &'a Path,
    /// Report format.
    pub format: &'a str,
    /// Output file.
    pub out: &'a Path,
    /// Comma-separated sections.
    pub include: Option<&'a str>,
}

/// Runs `freespace export`.
///
/// # Errors
/// Returns an error for invalid formats or sections, when the scan fails, or
/// when the report cannot be written.
pub fn run(args: ExportArgs<'_>, units: Units) -> Result<Outcome> {
    let format = ReportFormat::parse_value(args.format).ok_or_else(|| {
        Error::InvalidArgument(format!(
            "unknown report format `{}`; expected csv, xml, html or xlsx",
            args.format
        ))
    })?;
    let include = parse_include(args.include)?;
    let mut result = execute(ScanArgs {
        path: args.path,
        method: "auto",
        incremental: false,
        exclusions: &[],
    })?;

    let owner_scan = if include.contains(&SectionName::Owners) {
        Some(annotate(&mut result.tree)?)
    } else {
        None
    };
    let top = include
        .contains(&SectionName::Top)
        .then(|| top_files(&result.tree, EXPORT_TOP_LIMIT));
    let types = include
        .contains(&SectionName::Types)
        .then(|| group_by_extension(&result.tree));
    let duplicates = if include.contains(&SectionName::Duplicates) {
        let mut search = find_duplicates(&result.tree, DuplicateOptions::default(), None)?;
        result.warnings.append(&mut search.warnings);
        Some(search)
    } else {
        None
    };

    let roots: Vec<String> = result
        .session
        .roots
        .iter()
        .map(|root| paths::display(root))
        .collect();
    let meta = ReportMeta::new(Timestamp::now(), env!("CARGO_PKG_VERSION"))
        .with_volume(ReportVolume::from_volume(&result.volume))
        .with_roots(roots)
        .with_session(result.session.id.to_string())
        .with_units(units.as_str());
    let totals = result
        .tree
        .roots()
        .map(|root| result.tree.node(root).size)
        .sum();

    let mut report = Report::new(meta, totals)
        .with_include(include.clone())
        .with_tree(&result.tree);
    if let Some(top) = &top {
        report = report.with_top(top);
    }
    if let Some(types) = &types {
        report = report.with_types(types);
    }
    if let Some(search) = &duplicates {
        report = report.with_duplicates(&search.groups);
    }
    if let Some(scan) = &owner_scan {
        report = report.with_owners(&scan.summaries, &scan.table);
    }

    let row_count = report.total_rows();
    write_report(&report, format, args.out)?;

    let mut warnings = result.warnings;
    if let Some(scan) = &owner_scan {
        if scan.error_count > 0 {
            warnings.push(format!(
                "{} files could not be inspected for ownership",
                scan.error_count
            ));
        }
    }

    Ok(Outcome::new(
        json!({
            "path": paths::display(args.out),
            "format": format.as_str(),
            "sections": report.sections().iter().map(|section| section.as_str()).collect::<Vec<_>>(),
            "row_count": row_count,
            "total_bytes": report.totals.logical,
            "on_disk_bytes": report.totals.on_disk,
            "physical_bytes": report.totals.physical,
        }),
        format!(
            "report written: {} ({} rows, {} bytes)\n",
            paths::display(args.out),
            row_count,
            report.totals.logical
        ),
    )
    .with_warnings(warnings))
}

fn parse_include(include: Option<&str>) -> Result<Vec<SectionName>> {
    let Some(value) = include else {
        return Ok(SectionName::ALL.to_vec());
    };
    let mut sections = Vec::new();
    for part in value.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let section = SectionName::parse_value(part).ok_or_else(|| {
            Error::InvalidArgument(format!(
                "unknown report section `{part}`; expected tree, top, types, duplicates or owners"
            ))
        })?;
        if !sections.contains(&section) {
            sections.push(section);
        }
    }
    if sections.is_empty() {
        return Err(Error::InvalidArgument(
            "`--include` did not name a report section".to_owned(),
        ));
    }

    Ok(sections)
}

#[cfg(test)]
mod tests {
    use freespace_core::size::Units;

    use super::{ExportArgs, parse_include, run};

    fn fixture() -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("temp dir");
        std::fs::write(directory.path().join("a.txt"), [0u8; 10]).expect("a");
        std::fs::write(directory.path().join("b.bin"), [0u8; 20]).expect("b");
        directory
    }

    #[test]
    fn parses_sections() {
        let sections = parse_include(Some("tree, top,types")).expect("sections");

        assert_eq!(sections.len(), 3);
        assert!(parse_include(Some("unknown")).is_err());
        assert!(parse_include(Some(" , ")).is_err());
        assert_eq!(parse_include(None).expect("default").len(), 5);
    }

    #[test]
    fn exports_every_format_consistently() {
        let directory = fixture();
        let output = tempfile::tempdir().expect("output");

        for format in ["csv", "xml", "html", "xlsx"] {
            let path = output.path().join(format!("report.{format}"));
            let outcome = run(
                ExportArgs {
                    path: directory.path(),
                    format,
                    out: &path,
                    include: Some("tree,top,types"),
                },
                Units::Binary,
            )
            .expect("export");

            assert!(path.exists(), "{format} file");
            assert_eq!(outcome.data["format"], format);
            assert!(outcome.data["row_count"].as_u64().unwrap_or(0) >= 3);
            assert!(outcome.data["total_bytes"].as_u64().unwrap_or(0) >= 30);
        }
    }

    #[test]
    fn rejects_unknown_formats() {
        let directory = fixture();
        let output = tempfile::tempdir().expect("output");
        let path = output.path().join("report.pdf");
        let error = run(
            ExportArgs {
                path: directory.path(),
                format: "pdf",
                out: &path,
                include: None,
            },
            Units::Binary,
        )
        .expect_err("format");

        assert_eq!(error.code().as_str(), "E_INVALID_ARGUMENT");
    }
}
