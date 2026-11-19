//! US17: CSV, XML, HTML and XLSX export with consistent totals.

mod common;

use std::{io::Read as _, path::PathBuf};

use freespace_analysis::{
    DuplicateGroup, DuplicateMember, HashAlgorithm, OwnerSummary, OwnerTable, TopFile,
    group_by_extension, top_files,
};
use freespace_core::{
    Timestamp, Uuid,
    size::{SizeMetrics, Units},
    tree::{FsTree, NodeKind, NodeSpec},
};
use freespace_report::{Report, ReportFormat, ReportMeta, SectionName, write_report};

struct Fixture {
    directory: tempfile::TempDir,
    tree: FsTree,
    top: Vec<TopFile>,
    types: Vec<freespace_analysis::TypeGroup>,
    duplicates: Vec<DuplicateGroup>,
    owners: Vec<OwnerSummary>,
    table: OwnerTable,
}

fn fixture() -> Fixture {
    let directory = tempfile::tempdir().expect("temp dir");
    let mut tree = FsTree::new();
    let root = tree
        .add_node(
            None,
            NodeSpec {
                name: r"C:\".to_owned(),
                kind: NodeKind::Directory,
                ..NodeSpec::default()
            },
        )
        .expect("root");
    let first = tree
        .add_node(
            Some(root),
            NodeSpec {
                name: "<evil>.txt".to_owned(),
                kind: NodeKind::File,
                size: SizeMetrics::file(10),
                timestamps: freespace_core::tree::Timestamps {
                    modified: Timestamp::from_millis(1_700_000_000_000),
                    ..Default::default()
                },
                ..NodeSpec::default()
            },
        )
        .expect("first");
    let second = tree
        .add_node(
            Some(root),
            NodeSpec {
                name: "copy.bin".to_owned(),
                kind: NodeKind::File,
                size: SizeMetrics::file(20),
                ..NodeSpec::default()
            },
        )
        .expect("second");
    tree.recompute_totals();
    tree.node_mut(first).owner_id = Some(0);
    tree.node_mut(second).owner_id = Some(0);

    let mut table = OwnerTable::new();
    let owner_id = table.intern("S-1-5-21-1", r"PC\Alice");
    assert_eq!(owner_id, 0);

    let top = top_files(&tree, 10);
    let types = group_by_extension(&tree);
    let duplicates = vec![DuplicateGroup {
        id: Uuid::nil(),
        algorithm: HashAlgorithm::Sha256,
        hash: "deadbeef".to_owned(),
        size: 10,
        members: vec![
            DuplicateMember {
                node_id: first,
                path: PathBuf::from(r"C:\<evil>.txt"),
                size: 10,
                modified: Timestamp::UNIX_EPOCH,
                created: Timestamp::UNIX_EPOCH,
                selected_for_deletion: false,
                verified: true,
                reason: "keep".to_owned(),
            },
            DuplicateMember {
                node_id: second,
                path: PathBuf::from(r"C:\copy.bin"),
                size: 10,
                modified: Timestamp::UNIX_EPOCH,
                created: Timestamp::UNIX_EPOCH,
                selected_for_deletion: true,
                verified: true,
                reason: "delete".to_owned(),
            },
        ],
        keep_node_id: Some(first),
        reclaimable_bytes: 10,
        stale: false,
    }];
    let owners = vec![OwnerSummary {
        owner_id,
        sid: "S-1-5-21-1".to_owned(),
        display_name: r"PC\Alice".to_owned(),
        total_bytes: 30,
        file_count: 2,
    }];

    Fixture {
        directory,
        tree,
        top,
        types,
        duplicates,
        owners,
        table,
    }
}

fn report(fixture: &Fixture) -> Report<'_> {
    let meta = ReportMeta::new(Timestamp::from_millis(1_700_000_000_000), "1.0.0")
        .with_roots([r"C:\".to_owned()])
        .with_session("session")
        .with_units("binary");
    Report::new(meta, fixture.tree.node(0).size)
        .with_tree(&fixture.tree)
        .with_top(&fixture.top)
        .with_types(&fixture.types)
        .with_duplicates(&fixture.duplicates)
        .with_owners(&fixture.owners, &fixture.table)
}

#[test]
fn exports_csv_with_bom_sections_and_consistent_totals() {
    let fixture = fixture();
    let report = report(&fixture);
    let path = fixture.directory.path().join("report.csv");

    write_report(&report, ReportFormat::Csv, &path).expect("csv");

    let bytes = std::fs::read(&path).expect("read");
    assert_eq!(&bytes[..3], &[0xEF, 0xBB, 0xBF]);
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(&bytes[3..]);
    let records: Vec<csv::StringRecord> =
        reader.records().map(|row| row.expect("record")).collect();

    let section = records
        .iter()
        .position(|row| row.get(0) == Some("# section: tree"))
        .expect("tree section");
    assert_eq!(records[section + 1].get(3), Some("logical_size"));
    let root_row = records
        .iter()
        .skip(section + 2)
        .find(|row| row.get(1) == Some("C:\\"))
        .expect("root row");
    assert_eq!(root_row.get(3), Some("30"));
    assert_eq!(report.totals.logical, 30);
    assert_eq!(report.row_count(SectionName::Tree), 3);
}

#[test]
fn exports_xml_and_escapes_user_paths() {
    let fixture = fixture();
    let report = report(&fixture);
    let path = fixture.directory.path().join("report.xml");

    write_report(&report, ReportFormat::Xml, &path).expect("xml");
    let text = std::fs::read_to_string(&path).expect("read");

    assert!(text.contains("<freespace-report version=\"1\">"));
    assert!(text.contains("<section name=\"tree\">"));
    assert!(text.contains("<node "));
    assert!(text.contains("&lt;evil&gt;.txt"));
    assert!(!text.contains("<evil>.txt"));
    assert!(text.contains("<totals logical-size=\"30\""));
}

#[test]
fn exports_self_contained_html_with_escaping() {
    let fixture = fixture();
    let report = report(&fixture);
    let path = fixture.directory.path().join("report.html");

    write_report(&report, ReportFormat::Html, &path).expect("html");
    let text = std::fs::read_to_string(&path).expect("read");

    assert!(text.starts_with("<!DOCTYPE html>"));
    assert!(text.contains("<style>"));
    assert!(text.contains("Summary"));
    assert!(text.contains("&lt;evil&gt;.txt"));
    assert!(!text.contains("<evil>.txt"));
    assert!(!text.contains("http://"));
}

#[test]
fn exports_xlsx_with_the_documented_sheets() {
    let fixture = fixture();
    let report = report(&fixture);
    let path = fixture.directory.path().join("report.xlsx");

    write_report(&report, ReportFormat::Xlsx, &path).expect("xlsx");
    let file = std::fs::File::open(&path).expect("open");
    let mut archive = zip::ZipArchive::new(file).expect("xlsx is a zip");

    let mut workbook = String::new();
    archive
        .by_name("xl/workbook.xml")
        .expect("workbook")
        .read_to_string(&mut workbook)
        .expect("read workbook");
    for sheet in [
        "Summary",
        "Tree",
        "TopFiles",
        "Types",
        "Duplicates",
        "Owners",
    ] {
        assert!(workbook.contains(sheet), "missing sheet {sheet}");
    }
    assert!(archive.by_name("xl/worksheets/sheet1.xml").is_ok());
    assert!(archive.by_name("xl/worksheets/sheet6.xml").is_ok());
}

#[test]
fn the_cli_export_writes_every_format() {
    let fixture = common::TestTree::new();
    fixture.write("a.txt", &[0u8; 10]);
    fixture.write("b.bin", &[0u8; 20]);
    let output = tempfile::tempdir().expect("output");

    for format in ["csv", "xml", "html", "xlsx"] {
        let path = output.path().join(format!("report.{format}"));
        let outcome = freespace_cli::commands::export::run(
            freespace_cli::commands::export::ExportArgs {
                path: fixture.root(),
                format,
                out: &path,
                include: Some("tree,top,types"),
            },
            Units::Binary,
        )
        .expect("export");

        assert!(path.exists(), "{format}");
        assert_eq!(outcome.data["format"], format);
        assert!(outcome.data["row_count"].as_u64().unwrap_or(0) >= 3);
        assert!(outcome.data["total_bytes"].as_u64().unwrap_or(0) >= 30);
    }
}

#[test]
fn parses_format_names() {
    assert_eq!(ReportFormat::parse_value("CSV"), Some(ReportFormat::Csv));
    assert_eq!(ReportFormat::parse_value("excel"), Some(ReportFormat::Xlsx));
    assert_eq!(ReportFormat::parse_value("pdf"), None);
    assert_eq!(SectionName::parse_value("top"), Some(SectionName::Top));
}
