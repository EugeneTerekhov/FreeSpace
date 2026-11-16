//! Contract checks for the core data path (US1).
//!
//! The commands themselves are thin Tauri adapters; this test drives the same
//! `freespace-app` services against a temporary directory and verifies the
//! documented invariants and fixture shapes.

use std::fs;

use freespace_app::{
    dto::{FilterSet, Metric, SortKey, SortSpec, TreemapRequest},
    scan::run_scan,
    tree::TreeSnapshot,
    treemap,
};
use freespace_scan::session::NoopObserver;
use serde_json::Value;

fn fixture(name: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("contract/fixtures")
        .join(name);
    serde_json::from_str(&fs::read_to_string(path).expect("fixture is readable"))
        .expect("fixture is valid JSON")
}

fn scanned() -> (tempfile::TempDir, TreeSnapshot) {
    let directory = tempfile::tempdir().expect("temp dir");
    fs::create_dir(directory.path().join("alpha")).expect("dir");
    fs::create_dir(directory.path().join("beta")).expect("dir");
    fs::write(directory.path().join("alpha").join("report.pdf"), b"pdf").expect("file");
    fs::write(directory.path().join("alpha").join("photo.jpg"), [b'i'; 32]).expect("file");
    fs::write(
        directory.path().join("beta").join("archive.zip"),
        [b'z'; 64],
    )
    .expect("file");
    fs::write(directory.path().join("notes.txt"), b"notes").expect("file");

    let request = freespace_app::dto::ScanRequest {
        path: directory.path().to_string_lossy().into_owned(),
        method: freespace_app::dto::ScanMethodChoice::Walk,
        incremental: false,
        exclusions: Vec::new(),
        include_ads: false,
        metric: Metric::OnDisk,
    };
    let outcome = run_scan(&request, &mut NoopObserver).expect("scan");
    let snapshot = TreeSnapshot::new(outcome.tree, 1, outcome.volume, Some(outcome.session));

    (directory, snapshot)
}

#[test]
fn scan_produces_a_paged_tree() {
    let (_directory, snapshot) = scanned();
    assert!(snapshot.node_count() >= 6);

    let roots = snapshot
        .root_page(
            Metric::OnDisk,
            SortSpec {
                key: SortKey::Size,
                descending: true,
            },
            500,
            None,
        )
        .expect("roots");
    assert_eq!(roots.generation, 1);
    assert_eq!(roots.parent_id, None);
    assert_eq!(roots.rows.len(), 1);
    assert_eq!(roots.rows[0].kind, freespace_app::dto::NodeKind::Directory);

    let root_id = roots.rows[0].id;
    let children = snapshot
        .children_page(
            root_id,
            Metric::OnDisk,
            SortSpec::default(),
            &FilterSet::default(),
            2,
            None,
        )
        .expect("children");
    assert_eq!(children.rows.len(), 2);
    assert!(children.next_cursor.is_some());

    let next = snapshot
        .children_page(
            root_id,
            Metric::OnDisk,
            SortSpec::default(),
            &FilterSet::default(),
            10,
            children.next_cursor.as_deref(),
        )
        .expect("next page");
    assert!(!next.rows.is_empty());
    assert!(next.next_cursor.is_none());
}

#[test]
fn search_and_ancestors_work_on_a_generation() {
    let (_directory, snapshot) = scanned();

    let found = snapshot
        .search_page(
            "report",
            Metric::OnDisk,
            SortSpec::default(),
            &FilterSet::default(),
            10,
            None,
        )
        .expect("search");
    assert_eq!(found.rows.len(), 1);
    assert_eq!(found.rows[0].name, "report.pdf");

    let ancestors = snapshot.path_rows(found.rows[0].id).expect("ancestors");
    assert_eq!(ancestors.last().map(|row| row.name.as_str()), Some("alpha"));
}

#[test]
fn treemap_respects_the_tile_budget_and_totals() {
    let (_directory, snapshot) = scanned();
    let response = treemap::tiles(
        &snapshot,
        &TreemapRequest {
            width: 800,
            height: 600,
            max_tiles: 100,
            ..TreemapRequest::default()
        },
    )
    .expect("tiles");

    assert!(!response.tiles.is_empty());
    assert!(response.tiles.len() <= 100);
    let sum: u64 = response.tiles.iter().map(|tile| tile.size).sum();
    assert!(sum <= response.total_size);
    assert!(
        response
            .tiles
            .iter()
            .all(|tile| tile.rect.w > 0.0 && tile.rect.h > 0.0)
    );
}

#[test]
fn fixtures_match_the_serialised_dtos() {
    let (_directory, snapshot) = scanned();
    let page = snapshot
        .root_page(Metric::OnDisk, SortSpec::default(), 500, None)
        .expect("page");
    let page_value = serde_json::to_value(&page).expect("serialise page");
    let root_fixture = fixture("tree_root.success.json");
    let expected_keys: std::collections::BTreeSet<&str> = root_fixture["result"]
        .as_object()
        .expect("fixture result")
        .keys()
        .map(String::as_str)
        .collect();
    let actual_keys: std::collections::BTreeSet<&str> = page_value
        .as_object()
        .expect("page object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(actual_keys, expected_keys);

    let tiles = treemap::tiles(&snapshot, &TreemapRequest::default()).expect("tiles");
    let tiles_value = serde_json::to_value(&tiles).expect("serialise tiles");
    let tiles_fixture = fixture("treemap_tiles.success.json");
    let expected_keys: std::collections::BTreeSet<&str> = tiles_fixture["result"]
        .as_object()
        .expect("fixture result")
        .keys()
        .map(String::as_str)
        .collect();
    let actual_keys: std::collections::BTreeSet<&str> = tiles_value
        .as_object()
        .expect("tiles object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(actual_keys, expected_keys);
}
