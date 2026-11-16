//! US22: volume summary and snapshot trend on the dashboard.

mod common;

use std::collections::HashMap;
use std::path::PathBuf;

use freespace_core::{
    Timestamp, Uuid,
    dashboard::{Dashboard, SnapshotPoint, VolumeSummary, VolumeTrend},
    model::{FileSystem, Volume, VolumeStatus},
    size::SizeMetrics,
    tree::{FsTree, NodeKind, NodeSpec},
};
use freespace_platform::volumes;
use freespace_report::{SnapshotPayload, SnapshotSettings, from_tree};

fn volume(letter: char, total: u64, free: u64) -> Volume {
    let mount_point = PathBuf::from(format!("{letter}:\\"));

    Volume {
        id: Volume::id_for_mount_point(&mount_point),
        mount_point,
        label: format!("Volume {letter}"),
        file_system: FileSystem::Ntfs,
        total_bytes: total,
        free_bytes: free,
        cluster_size: Some(4096),
        scan_method: None,
        is_elevated: false,
        is_removable: false,
        status: VolumeStatus::Available,
    }
}

#[test]
fn summarises_every_real_volume() {
    let dashboard = volumes::dashboard().expect("dashboard");

    assert!(!dashboard.volumes.is_empty());
    for summary in &dashboard.volumes {
        assert!(summary.used_bytes <= summary.volume.total_bytes);
        assert!(summary.used_percent <= 100);
        assert_eq!(
            summary.is_available(),
            summary.volume.status == VolumeStatus::Available
        );
    }
    let totals = dashboard.totals();
    assert_eq!(totals.volumes, dashboard.volumes.len());
    assert!(totals.used_bytes <= totals.total_bytes);
}

#[test]
fn computes_usage_from_the_domain_model() {
    let summary = VolumeSummary::new(volume('C', 1_000_000, 250_000));

    assert_eq!(summary.used_bytes, 750_000);
    assert_eq!(summary.used_percent, 75);
    assert!((summary.usage_ratio() - 0.75).abs() < 0.001);
}

#[test]
fn derives_dynamics_from_stored_snapshots() {
    let directory = tempfile::tempdir().expect("temp dir");
    let target = volume('T', 10_000, 6_000);

    let mut tree = FsTree::new();
    let root = tree
        .add_node(
            None,
            NodeSpec {
                name: r"T:\".to_owned(),
                kind: NodeKind::Directory,
                ..NodeSpec::default()
            },
        )
        .expect("root");
    tree.add_node(
        Some(root),
        NodeSpec {
            name: "data.bin".to_owned(),
            kind: NodeKind::File,
            size: SizeMetrics::file(4_000),
            ..NodeSpec::default()
        },
    )
    .expect("file");
    tree.recompute_totals();

    let mut trends: HashMap<Uuid, Vec<SnapshotPoint>> = HashMap::new();
    for (index, free) in [6_000u64, 5_000, 3_000].into_iter().enumerate() {
        let mut payload = from_tree(
            &tree,
            &target,
            std::slice::from_ref(&target.mount_point),
            SnapshotSettings::default(),
            Timestamp::from_millis(1_700_000_000_000 + index as i64 * 86_400_000),
        )
        .expect("snapshot");
        payload.volume.free_bytes = free;
        let path = directory.path().join(format!("snap-{index}.fssnap"));
        payload.write(&path).expect("write snapshot");

        let stored = SnapshotPayload::read(&path).expect("read snapshot");
        let taken_at = Timestamp::parse_iso8601(&stored.created_at).expect("timestamp");
        trends
            .entry(Volume::id_for_mount_point(std::path::Path::new(
                &stored.volume.mount_point,
            )))
            .or_default()
            .push(SnapshotPoint::new(
                taken_at,
                stored.volume.total_bytes - stored.volume.free_bytes,
                stored.volume.free_bytes,
            ));
    }

    let mut dashboard = Dashboard::from_volumes(vec![target.clone(), volume('U', 1, 1)]);
    dashboard.attach_trends(&trends);

    let summary = dashboard.find_by_id(target.id).expect("volume T");
    assert_eq!(summary.trend.points.len(), 3);
    assert_eq!(summary.trend.change_bytes(), 3_000);
    assert!(summary.trend.growth_per_day() > 0.0);
    assert!(dashboard.volumes[1].trend.is_empty());
}

#[test]
fn trends_are_sorted_deduplicated_and_bounded() {
    let mut points: Vec<SnapshotPoint> = (0..300u64)
        .map(|index| {
            SnapshotPoint::new(
                Timestamp::from_millis(i64::try_from(index).unwrap_or(0)),
                index,
                0,
            )
        })
        .collect();
    points.push(SnapshotPoint::new(Timestamp::from_millis(299), 999, 0));

    let trend = VolumeTrend::from_points(points);

    assert_eq!(
        trend.points.len(),
        freespace_core::dashboard::MAX_TREND_POINTS
    );
    let timestamps: Vec<i64> = trend
        .points
        .iter()
        .map(|point| point.taken_at.as_millis())
        .collect();
    assert!(timestamps.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(trend.points.last().expect("latest").used_bytes, 299);
    assert_eq!(trend.change_bytes(), 299 - 44);
}
