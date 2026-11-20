#![cfg(windows)]

//! US15: owner aggregation, the tree filter and the owner report section.

mod common;

use freespace_analysis::{OwnerIdentity, UNKNOWN_OWNER, annotate_with, summarize_owners};
use freespace_core::{
    query::{FilterSpec, SortDirection, SortKey, SortSpec, search},
    size::{SizeMetrics, Units},
    tree::{FsTree, NodeKind, NodeSpec},
};
use freespace_report::OwnerReport;

fn fixture() -> (FsTree, u32) {
    let mut tree = FsTree::new();
    let root = tree
        .add_node(
            None,
            NodeSpec {
                name: r"C:\data".to_owned(),
                kind: NodeKind::Directory,
                ..NodeSpec::default()
            },
        )
        .expect("root");
    for (name, size) in [("alice.bin", 100u64), ("bob.bin", 250), ("shared.bin", 50)] {
        tree.add_node(
            Some(root),
            NodeSpec {
                name: name.to_owned(),
                kind: NodeKind::File,
                size: SizeMetrics::file(size),
                ..NodeSpec::default()
            },
        )
        .expect("file");
    }
    tree.recompute_totals();

    (tree, root)
}

fn identity(sid: &str, name: &str) -> Option<OwnerIdentity> {
    Some(OwnerIdentity {
        sid: sid.to_owned(),
        display_name: name.to_owned(),
    })
}

#[test]
fn aggregates_sizes_per_owner_and_keeps_unknown() {
    let (mut tree, _) = fixture();
    let scan = annotate_with(&mut tree, |path: &std::path::Path| {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        match name {
            "alice.bin" => Ok(identity("S-1-5-21-1", r"PC\Alice")),
            "bob.bin" => Ok(identity("S-1-5-21-2", r"PC\Bob")),
            _ => Ok(None),
        }
    })
    .expect("annotate");

    assert_eq!(scan.table.len(), 2);
    assert_eq!(scan.unknown_count, 1);
    assert_eq!(scan.summaries[0].display_name, r"PC\Bob");
    assert_eq!(scan.summaries[0].total_bytes, 250);
    assert_eq!(scan.summaries[0].file_count, 1);
    assert_eq!(scan.summaries[2].display_name, UNKNOWN_OWNER);
    assert_eq!(scan.total_bytes(), 400);
    assert_eq!(summarize_owners(&tree, &scan.table), scan.summaries);
}

#[test]
fn filters_the_tree_by_owner_index() {
    let (mut tree, root) = fixture();
    let scan = annotate_with(&mut tree, |path: &std::path::Path| {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        match name {
            "alice.bin" => Ok(identity("S-1-5-21-1", r"PC\Alice")),
            "bob.bin" => Ok(identity("S-1-5-21-2", r"PC\Bob")),
            _ => Ok(None),
        }
    })
    .expect("annotate");
    let alice = scan.table.find(r"PC\Alice").expect("alice");

    let filter = FilterSpec {
        owner_ids: vec![alice],
        include_hidden: true,
        ..FilterSpec::default()
    }
    .compile()
    .expect("filter");
    let matched = search(
        &tree,
        root,
        &filter,
        SortSpec::new(SortKey::Name, SortDirection::Ascending),
        false,
        10,
    );
    assert!(matched.iter().any(|id| tree.name(*id) == "alice.bin"));
    assert!(!matched.iter().any(|id| tree.name(*id) == "bob.bin"));
    assert!(
        matched.contains(&root),
        "directories remain as containers in the filtered tree"
    );

    let unknown = FilterSpec {
        owner_ids: vec![u32::MAX],
        include_hidden: true,
        ..FilterSpec::default()
    }
    .compile()
    .expect("unknown filter");
    let matched = search(&tree, root, &unknown, SortSpec::default(), false, 10);
    assert!(matched.iter().any(|id| tree.name(*id) == "shared.bin"));
    assert!(!matched.iter().any(|id| tree.name(*id) == "alice.bin"));
}

#[test]
fn builds_the_report_section() {
    let (mut tree, _) = fixture();
    let scan = annotate_with(&mut tree, |path: &std::path::Path| {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        match name {
            "alice.bin" => Ok(identity("S-1-5-21-1", r"PC\Alice")),
            "bob.bin" => Ok(identity("S-1-5-21-2", r"PC\Bob")),
            _ => Ok(None),
        }
    })
    .expect("annotate");
    let report = OwnerReport::from_summaries(scan.summaries);

    assert_eq!(report.row_count(), 3);
    assert_eq!(report.total_bytes(), 400);
    assert_eq!(report.file_count(), 3);
    assert!((report.share(250) - 0.625).abs() < f64::EPSILON);
}

#[test]
fn the_cli_lists_the_current_owner() {
    let fixture = common::TestTree::new();
    fixture.write("a.bin", &[0u8; 10]);
    fixture.write("b.bin", &[0u8; 20]);

    let outcome =
        freespace_cli::commands::owners::run(fixture.root(), Units::Binary).expect("owners");
    let items = outcome.data["items"].as_array().expect("items");

    assert!(!items.is_empty());
    assert_eq!(outcome.data["total_bytes"].as_u64(), Some(30));
    assert!(items.iter().all(|item| {
        item["sid"]
            .as_str()
            .is_some_and(|sid| sid.starts_with("S-1-"))
    }));
}
