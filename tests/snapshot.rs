//! US8: `.fssnap` round-trip and snapshot comparison.

mod common;

use std::path::PathBuf;

use freespace_core::{
    Timestamp,
    model::{FileSystem, Volume, VolumeStatus},
    size::SizeMetrics,
    tree::{FsTree, NodeKind, NodeSpec, Timestamps},
};
use freespace_report::{SnapshotPayload, SnapshotSettings, compare, from_tree};

fn volume(letter: char) -> Volume {
    let mount_point = PathBuf::from(format!("{letter}:\\"));
    Volume {
        id: Volume::id_for_mount_point(&mount_point),
        mount_point,
        label: "Fixture".to_owned(),
        file_system: FileSystem::Ntfs,
        total_bytes: 10_000_000,
        free_bytes: 5_000_000,
        cluster_size: Some(4_096),
        scan_method: None,
        is_elevated: false,
        is_removable: false,
        status: VolumeStatus::Available,
    }
}

fn snapshot(files: &[(&str, u64)], volume: &Volume) -> SnapshotPayload {
    let mut tree = FsTree::new();
    let root = tree
        .add_node(
            None,
            NodeSpec {
                name: volume.mount_point.to_string_lossy().into_owned(),
                kind: NodeKind::Directory,
                ..NodeSpec::default()
            },
        )
        .expect("root");
    let data = tree
        .add_node(
            Some(root),
            NodeSpec {
                name: "data".to_owned(),
                kind: NodeKind::Directory,
                ..NodeSpec::default()
            },
        )
        .expect("data");

    for (name, size) in files {
        tree.add_node(
            Some(data),
            NodeSpec {
                name: (*name).to_owned(),
                kind: NodeKind::File,
                size: SizeMetrics::file(*size),
                timestamps: Timestamps {
                    modified: Timestamp::from_millis(1_700_000_000_000),
                    ..Timestamps::default()
                },
                ..NodeSpec::default()
            },
        )
        .expect("file");
    }
    tree.recompute_totals();

    from_tree(
        &tree,
        volume,
        std::slice::from_ref(&volume.mount_point),
        SnapshotSettings::default(),
        Timestamp::from_millis(1_700_000_000_000),
    )
    .expect("payload")
}

#[test]
fn writes_reads_and_verifies_a_snapshot() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("disk.fssnap");
    let payload = snapshot(&[("a.bin", 10), ("b.bin", 20)], &volume('C'));

    payload.write(&path).expect("write");
    let loaded = SnapshotPayload::read(&path).expect("read");

    assert_eq!(loaded.volume.id, payload.volume.id);
    assert_eq!(loaded.node_count, payload.node_count);
    assert_eq!(loaded.nodes.len(), 4);
    assert_eq!(loaded.roots.len(), 1);
    assert_eq!(loaded.format_version, 1);
    assert_eq!(
        loaded.compute_checksum().expect("checksum"),
        loaded.checksum
    );
}

#[test]
fn rejects_a_tampered_snapshot() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("disk.fssnap");
    let payload = snapshot(&[("a.bin", 10)], &volume('C'));
    payload.write(&path).expect("write");

    let mut bytes = std::fs::read(&path).expect("read bytes");
    let last = bytes.len() - 1;
    bytes[last] ^= 0xFF;
    std::fs::write(&path, bytes).expect("tamper");

    let error = SnapshotPayload::read(&path).expect_err("must be corrupted");
    assert_eq!(error.code().as_str(), "E_CORRUPTED_SNAPSHOT");
}

#[test]
fn compares_two_snapshots_and_sorts_growth() {
    let volume = volume('C');
    let base = snapshot(&[("keep.bin", 100), ("gone.bin", 50)], &volume);
    let target = snapshot(&[("keep.bin", 300), ("new.bin", 500)], &volume);

    let diff = compare(&base, &target).expect("compare");

    assert_eq!(diff.added.len(), 1);
    assert_eq!(diff.removed.len(), 1);
    assert_eq!(diff.changed.len(), 2, "the data directory grew too");
    assert!(
        diff.changed
            .iter()
            .any(|entry| entry.path == "data\\keep.bin" && entry.size_delta == 200)
    );
    assert_eq!(diff.total_growth_bytes, 650);
    assert_eq!(diff.growth_by_directory[0].path, "data");
    assert_eq!(diff.growth_by_directory[0].delta, 650);
}

#[test]
fn refuses_snapshots_of_different_volumes() {
    let base = snapshot(&[("a.bin", 10)], &volume('C'));
    let target = snapshot(&[("a.bin", 10)], &volume('D'));

    let error = compare(&base, &target).expect_err("must refuse");

    assert_eq!(error.code().as_str(), "E_INCOMPATIBLE_SNAPSHOT");
}

#[test]
fn the_cli_creates_a_readable_snapshot() {
    let fixture = common::TestTree::new();
    fixture.write("payload.bin", &[1u8; 128]);
    let directory = tempfile::tempdir().expect("temp dir");
    let out = directory.path().join("cli.fssnap");

    let outcome = freespace_cli::commands::snapshot::create(
        fixture.root(),
        &out,
        freespace_core::size::Units::Binary,
    )
    .expect("create");
    assert_eq!(outcome.data["node_count"].as_u64(), Some(2));
    assert!(out.exists());

    let payload = SnapshotPayload::read(&out).expect("read");
    assert!(!payload.volume.mount_point.is_empty());
}
