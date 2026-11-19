//! US7: stale-file filters, cleanup recommendations and empty directories.

mod common;

use common::TestTree;
use freespace_analysis::{CleanupCategory, RecommendOptions, StaleOptions, find_stale, recommend};
use freespace_core::{
    Timestamp,
    size::SizeMetrics,
    time::MILLIS_PER_DAY,
    tree::{FsTree, NodeKind, NodeSpec, Timestamps},
};
use freespace_scan::{MethodChoice, NoopObserver, ScanRequest, find_empty_dirs, scan};

#[test]
fn filters_by_the_latest_of_access_and_modification() {
    let base = 1_700_000_000_000i64;
    let mut tree = FsTree::new();
    let root = tree
        .add_node(
            None,
            NodeSpec {
                name: "root".to_owned(),
                kind: NodeKind::Directory,
                ..NodeSpec::default()
            },
        )
        .expect("root");
    for (name, accessed, modified) in [
        ("fresh.txt", 390, 10),
        ("stale.txt", 1, 100),
        ("modified.txt", 1, 150),
    ] {
        tree.add_node(
            Some(root),
            NodeSpec {
                name: name.to_owned(),
                kind: NodeKind::File,
                size: SizeMetrics::file(10),
                timestamps: Timestamps {
                    accessed: Timestamp::from_millis(base + accessed * MILLIS_PER_DAY),
                    modified: Timestamp::from_millis(base + modified * MILLIS_PER_DAY),
                    created: Timestamp::from_millis(base),
                },
                ..NodeSpec::default()
            },
        )
        .expect("file");
    }
    tree.recompute_totals();

    let now = Timestamp::from_millis(base + 400 * MILLIS_PER_DAY);
    let stale = find_stale(&tree, now, StaleOptions::default());

    assert_eq!(stale.len(), 2);
    assert!(stale.iter().all(|entry| !entry.path.ends_with("fresh.txt")));
    let modified = stale
        .iter()
        .find(|entry| entry.path.ends_with("modified.txt"))
        .expect("modified file");
    assert!(
        !modified.accessed_reliable,
        "an access date older than the modification date cannot be trusted"
    );
}

#[test]
fn recommends_real_temp_and_cache_data() {
    let fixture = TestTree::new();
    fixture.write("Temp/scratch.tmp", &[0u8; 32]);
    fixture.write("Cache/blob.bin", &[1u8; 64]);
    fixture.write("photos/holiday.jpg", &[2u8; 128]);

    let request = ScanRequest {
        method: MethodChoice::Walk,
        ..ScanRequest::new(fixture.root())
    };
    let result = scan(&request, &mut NoopObserver).expect("scan");
    let candidates = recommend(&result.tree, RecommendOptions::default());

    assert!(
        candidates
            .iter()
            .any(|item| item.category == CleanupCategory::Temp)
    );
    assert!(
        candidates
            .iter()
            .any(|item| item.category == CleanupCategory::Cache)
    );
    assert!(
        candidates
            .iter()
            .all(|item| !item.path.ends_with("holiday.jpg"))
    );
}

#[test]
fn finds_every_empty_directory_including_nested_ones() {
    let fixture = TestTree::new();
    fixture.write("data/payload.bin", &[0u8; 16]);
    fixture.mkdir("empty");
    fixture.mkdir("empty/nested/deeper");

    let request = ScanRequest {
        method: MethodChoice::Walk,
        ..ScanRequest::new(fixture.root())
    };
    let result = scan(&request, &mut NoopObserver).expect("scan");
    let empty = find_empty_dirs(&result.tree, false);

    assert_eq!(empty.len(), 3, "all nested empty folders are reported");
    assert!(
        empty
            .iter()
            .all(|entry| !entry.path.ends_with("payload.bin"))
    );
    assert!(
        empty[0].depth >= empty[1].depth,
        "deepest directories come first"
    );
}
