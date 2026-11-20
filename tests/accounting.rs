#![cfg(windows)]

//! US10: hard-link de-duplication, reparse points and alternate data streams.

mod common;

use std::path::PathBuf;

use common::TestTree;
use freespace_analysis::{SizeMode, dedupe_physical, enrich_ads, summarize};
use freespace_core::{
    size::SizeMetrics,
    tree::{FsTree, NodeKind, NodeSpec},
};
use freespace_platform::{
    ads::list_streams,
    allocated::allocated_size,
    reparse::{ReparseKind, inspect},
};

fn accounting_tree(fixture: &TestTree, first: PathBuf, second: PathBuf) -> FsTree {
    let mut tree = FsTree::new();
    let root = tree
        .add_node(
            None,
            NodeSpec {
                name: fixture.root().to_string_lossy().into_owned(),
                kind: NodeKind::Directory,
                ..NodeSpec::default()
            },
        )
        .expect("root");
    for (name, path) in [("a.bin", first), ("b.bin", second)] {
        let size = std::fs::metadata(&path).expect("metadata").len();
        tree.add_node(
            Some(root),
            NodeSpec {
                name: name.to_owned(),
                kind: NodeKind::Hardlink,
                file_ref: 77,
                size: SizeMetrics::exact(size, size, size, 0),
                ..NodeSpec::default()
            },
        )
        .expect("link");
    }
    tree.recompute_totals();

    tree
}

#[test]
fn counts_hard_links_once_physically() {
    let fixture = TestTree::new();
    let first = fixture.write("first.bin", &[1u8; 256]);
    let second = fixture.path("second.bin");
    if std::fs::hard_link(&first, &second).is_err() {
        return;
    }
    let mut tree = accounting_tree(&fixture, first.clone(), second.clone());

    let before = summarize(&tree);
    assert_eq!(before.logical, 512);
    assert_eq!(before.physical, 256);
    assert_eq!(before.link_count, 1);

    let removed = dedupe_physical(&mut tree);
    assert_eq!(removed, 1);
    assert_eq!(summarize(&tree).physical, 256);
    assert_eq!(
        SizeMode::Physical.value(tree.node(root_id(&tree)).size),
        256
    );
}

fn root_id(tree: &FsTree) -> u32 {
    tree.roots().next().expect("root")
}

#[test]
fn reports_allocated_size_with_compression_savings() {
    let fixture = TestTree::new();
    let path = fixture.write("plain.bin", &[0u8; 16_384]);

    let allocated = allocated_size(&path).expect("allocated");
    assert!(allocated >= 16_384);
    assert_eq!(
        SizeMode::OnDisk.value(SizeMetrics::exact(16_384, allocated, allocated, 0)),
        allocated
    );
}

#[test]
fn enumerates_alternate_data_streams() {
    let fixture = TestTree::new();
    let path = fixture.write("host.bin", &[2u8; 64]);
    let stream_path = PathBuf::from(format!("{}:Zone.Identifier", path.display()));
    if std::fs::write(&stream_path, b"[ZoneTransfer]\r\nZoneId=3\r\n").is_err() {
        return;
    }

    let streams = list_streams(&path).expect("streams");
    assert!(
        streams
            .iter()
            .any(|stream| stream.name == "Zone.Identifier" && stream.size > 0)
    );
}

#[test]
fn recognises_a_junction_when_it_can_be_created() {
    let fixture = TestTree::new();
    let target = fixture.mkdir("target");
    let junction = fixture.path("junction");
    let status = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&junction)
        .arg(&target)
        .status();
    if !status.is_ok_and(|status| status.success()) {
        return;
    }

    let info = inspect(&junction).expect("inspect").expect("reparse point");

    assert_eq!(info.kind, ReparseKind::Junction);
}

#[test]
fn sparse_and_compressed_files_report_their_allocation() {
    let Some(_guard) = common::require_admin() else {
        return;
    };
    let fixture = TestTree::new();
    let sparse = fixture.write("sparse.bin", &[0u8; 1024 * 1024]);
    let compressed = fixture.write("compressed.bin", &[0x5Au8; 1024 * 1024]);

    let sparse_status = std::process::Command::new("fsutil")
        .args(["sparse", "setflag"])
        .arg(&sparse)
        .status();
    let range_status = std::process::Command::new("fsutil")
        .args(["sparse", "setrange"])
        .arg(&sparse)
        .args(["0", "1048576"])
        .status();
    let compressed_status = std::process::Command::new("compact")
        .arg("/c")
        .arg(&compressed)
        .status();
    if !sparse_status.is_ok_and(|status| status.success())
        || !range_status.is_ok_and(|status| status.success())
        || !compressed_status.is_ok_and(|status| status.success())
    {
        return;
    }

    let sparse_allocated = allocated_size(&sparse).expect("sparse allocation");
    let compressed_allocated = allocated_size(&compressed).expect("compressed allocation");

    assert!(sparse_allocated < 1024 * 1024);
    assert!(compressed_allocated < 1024 * 1024);
}

#[test]
fn classifies_a_symlink_when_available() {
    let fixture = TestTree::new();
    let target = fixture.write("target.txt", b"payload");
    let link = fixture.path("link.txt");
    if std::os::windows::fs::symlink_file(&target, &link).is_err() {
        return;
    }

    let info = inspect(&link).expect("inspect").expect("reparse point");

    assert_eq!(info.kind, ReparseKind::Symlink);
    assert!(info.target.is_some());
}

#[test]
fn enriches_ads_in_a_tree() {
    let fixture = TestTree::new();
    let path = fixture.write("host.bin", &[3u8; 32]);
    let stream_path = PathBuf::from(format!("{}:custom", path.display()));
    if std::fs::write(&stream_path, b"stream-data").is_err() {
        return;
    }

    let mut tree = FsTree::new();
    let root = tree
        .add_node(
            None,
            NodeSpec {
                name: fixture.root().to_string_lossy().into_owned(),
                kind: NodeKind::Directory,
                ..NodeSpec::default()
            },
        )
        .expect("root");
    tree.add_node(
        Some(root),
        NodeSpec {
            name: "host.bin".to_owned(),
            kind: NodeKind::File,
            size: SizeMetrics::file(32),
            ..NodeSpec::default()
        },
    )
    .expect("file");
    tree.recompute_totals();

    let report = enrich_ads(&mut tree);

    assert_eq!(report.files, 1);
    assert_eq!(report.streams, 1);
    assert_eq!(report.files_with_streams, 1);
}
