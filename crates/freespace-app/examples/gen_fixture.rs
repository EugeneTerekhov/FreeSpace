//! Synthetic tree generator and Treemap measurement harness.
//!
//! Usage:
//!   cargo run --release -p freespace-app --example gen_fixture -- 1000000
//!
//! The tree is built in memory with the same arena as a real scan. The tool
//! prints build time, name-arena size and the time to aggregate a 1600x900
//! Treemap with 2 000 tiles, which is the budget measured in
//! `docs/performance-002.md`.

use std::{env, mem::size_of, path::Path, time::Instant};

use freespace_app::{
    dto::{Metric, TreemapRequest},
    tree::TreeSnapshot,
    treemap,
};
use freespace_core::{
    model::{FileSystem, Volume, VolumeStatus},
    size::SizeMetrics,
    tree::{FsTree, NodeKind, NodeSpec},
};

fn main() {
    let nodes: usize = env::args()
        .nth(1)
        .and_then(|value| value.parse().ok())
        .unwrap_or(1_000_000);
    let directories = nodes.div_ceil(256).max(1);

    println!("nodes requested: {nodes}");
    println!("directories: {directories}");

    let build_started = Instant::now();
    let mut tree = FsTree::with_capacity(nodes.saturating_add(1), nodes.saturating_mul(24));
    let root = tree
        .add_node(
            None,
            NodeSpec {
                name: "C:\\".to_owned(),
                kind: NodeKind::Directory,
                ..NodeSpec::default()
            },
        )
        .expect("root");

    let mut directory = root;
    let mut files_in_directory = 0usize;
    for index in 0..nodes {
        if files_in_directory >= 256 {
            directory = tree
                .add_node(
                    Some(root),
                    NodeSpec {
                        name: format!("dir-{:06}", index / 256),
                        kind: NodeKind::Directory,
                        ..NodeSpec::default()
                    },
                )
                .expect("directory");
            files_in_directory = 0;
        }

        let size = 4_096 + u64::from((index as u32).wrapping_mul(7_919)) % 1_048_576;
        tree.add_node(
            Some(directory),
            NodeSpec {
                name: format!("file-{index:08}.bin"),
                kind: NodeKind::File,
                size: SizeMetrics::file(size),
                ..NodeSpec::default()
            },
        )
        .expect("file");
        files_in_directory += 1;
    }
    tree.recompute_totals();
    let build = build_started.elapsed();

    let volume = Volume {
        id: Volume::id_for_mount_point(Path::new("C:\\")),
        mount_point: "C:\\".into(),
        label: "Synthetic".to_owned(),
        file_system: FileSystem::Ntfs,
        total_bytes: 8 * 1024 * 1024 * 1024 * 1024,
        free_bytes: 4 * 1024 * 1024 * 1024 * 1024,
        cluster_size: Some(4_096),
        scan_method: None,
        is_elevated: false,
        is_removable: false,
        status: VolumeStatus::Available,
    };
    let snapshot = TreeSnapshot::new(tree, 1, volume, None);

    println!(
        "built: nodes={} name_arena_bytes={} build_ms={} fsnode_bytes={}",
        snapshot.node_count(),
        snapshot.tree().name_bytes(),
        build.as_millis(),
        size_of::<freespace_core::tree::FsNode>()
    );

    let request = TreemapRequest {
        width: 1_600,
        height: 900,
        metric: Metric::OnDisk,
        max_depth: 6,
        min_tile_px: 4,
        max_tiles: 2_000,
        node_id: None,
    };

    let layout_started = Instant::now();
    let response = treemap::tiles(&snapshot, &request).expect("treemap");
    let layout = layout_started.elapsed();

    println!(
        "treemap: tiles={} truncated={} total_bytes={} layout_ms={}",
        response.tiles.len(),
        response.truncated,
        response.total_size,
        layout.as_millis()
    );

    let root_page_started = Instant::now();
    let page = snapshot
        .root_page(
            Metric::OnDisk,
            freespace_app::dto::SortSpec::default(),
            500,
            None,
        )
        .expect("root page");
    let page_time = root_page_started.elapsed();

    println!(
        "tree_root: rows={} page_ms={}",
        page.rows.len(),
        page_time.as_millis()
    );
}
