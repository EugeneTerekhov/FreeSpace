//! Contract checks for the analysis command family (US2).
//!
//! The Tauri adapters are thin; these tests drive the same `freespace-app`
//! services against temporary directories and assert the documented invariants.

use std::{fs, path::Path};

use freespace_app::dto::{ScanMethodChoice, ScanRequest};
use freespace_app::{
    cleanup,
    dto::{
        FilterSet, Metric, SnapshotCreateRequest, SnapshotDiffSort, SnapshotDiffSortKey, SortSpec,
    },
    duplicates,
    scan::run_scan,
    snapshots::SnapshotService,
    top_types,
    tree::TreeSnapshot,
};
use freespace_scan::session::NoopObserver;

fn scanned(root: &Path, generation: u64) -> TreeSnapshot {
    let request = ScanRequest {
        path: root.to_string_lossy().into_owned(),
        method: ScanMethodChoice::Walk,
        incremental: false,
        exclusions: Vec::new(),
        include_ads: false,
        metric: Metric::OnDisk,
    };
    let outcome = run_scan(&request, &mut NoopObserver).expect("scan");

    TreeSnapshot::new(
        outcome.tree,
        generation,
        outcome.volume,
        Some(outcome.session),
    )
}

fn analysis_fixture() -> (tempfile::TempDir, TreeSnapshot) {
    let directory = tempfile::tempdir().expect("temp dir");
    fs::create_dir_all(directory.path().join("Cache")).expect("cache dir");
    fs::create_dir_all(directory.path().join("empty")).expect("empty dir");
    fs::write(directory.path().join("Cache").join("blob.tmp"), [1u8; 64]).expect("blob");
    fs::write(directory.path().join("large.mkv"), [2u8; 4096]).expect("video");
    fs::write(directory.path().join("report.pdf"), [3u8; 1024]).expect("pdf");
    fs::write(directory.path().join("notes.txt"), [4u8; 32]).expect("notes");
    fs::write(directory.path().join("duplicate-a.bin"), [9u8; 512]).expect("duplicate");
    fs::write(directory.path().join("duplicate-b.bin"), [9u8; 512]).expect("duplicate");

    let snapshot = scanned(directory.path(), 11);
    (directory, snapshot)
}

#[test]
fn top_files_and_types_report_native_totals() {
    let (_directory, snapshot) = analysis_fixture();

    let page = top_types::top_files_page(
        &snapshot,
        3,
        Metric::Logical,
        SortSpec::default(),
        &FilterSet::default(),
        10,
        None,
    )
    .expect("top page");
    assert_eq!(page.generation, 11);
    assert_eq!(page.rows.len(), 3);
    assert_eq!(page.rows[0].name, "large.mkv");

    let filter = FilterSet {
        extensions: vec!["pdf".to_owned()],
        ..FilterSet::default()
    };
    let filtered = top_types::top_files_page(
        &snapshot,
        100,
        Metric::Logical,
        SortSpec::default(),
        &filter,
        10,
        None,
    )
    .expect("filtered");
    assert_eq!(filtered.rows.len(), 1);
    assert_eq!(filtered.rows[0].name, "report.pdf");

    let summary = top_types::types_summary(&snapshot, &FilterSet::default()).expect("summary");
    let pdf = summary
        .iter()
        .find(|row| row.extension == "pdf")
        .expect("pdf row");
    assert_eq!(pdf.count, 1);
    assert_eq!(pdf.logical_size, 1024);
    assert_eq!(pdf.category, "document");

    let files = top_types::types_files_page(
        &snapshot,
        "category:video",
        Metric::Logical,
        SortSpec::default(),
        &FilterSet::default(),
        10,
        None,
    )
    .expect("video files");
    assert_eq!(files.rows.len(), 1);
    assert_eq!(files.rows[0].name, "large.mkv");

    let categories =
        top_types::types_category_summary(&snapshot, &FilterSet::default()).expect("categories");
    assert!(categories.iter().any(|row| row.category == "video"));
}

#[test]
fn duplicate_search_keeps_one_copy_and_pages_groups() {
    let (_directory, snapshot) = analysis_fixture();
    let search = duplicates::search(&snapshot, 1, "sha256", "oldest").expect("search");
    assert_eq!(search.groups.len(), 1);

    let page = duplicates::groups_page("task-1", &search, 10, None).expect("groups");
    assert_eq!(page.groups.len(), 1);
    assert_eq!(page.groups[0].members.len(), 2);
    assert!(page.groups[0].deletable);

    let all: Vec<String> = page.groups[0]
        .members
        .iter()
        .map(|member| member.path.clone())
        .collect();
    let error = duplicates::deletion_targets(&search, &all).expect_err("must refuse");
    assert_eq!(error.code, freespace_app::error::UiErrorCode::Validation);

    let targets = duplicates::deletion_targets(&search, &all[..1]).expect("one copy");
    assert_eq!(targets.len(), 1);

    let cursor = duplicates::groups_page("task-1", &search, 1, None);
    assert!(cursor.is_ok());
    let other = duplicates::groups_page("task-2", &search, 10, None).expect("other");
    assert_eq!(other.total_estimate, 1);
}

#[test]
fn cleanup_reports_candidates_and_empty_directories() {
    let (_directory, snapshot) = analysis_fixture();

    let candidates =
        cleanup::cleanup_candidates(&snapshot, 0, &FilterSet::default()).expect("candidates");
    assert!(
        candidates
            .iter()
            .any(|candidate| candidate.reason_key == "cleanup.reason.cache"),
        "cache is recommended"
    );
    assert!(
        candidates
            .iter()
            .all(|candidate| candidate.recommended_action != "delete" || candidate.risk == "low")
    );

    let empty = cleanup::empty_dirs_page(&snapshot, Metric::OnDisk, SortSpec::default(), 10, None)
        .expect("empty dirs");
    assert_eq!(empty.rows.len(), 1);
    assert_eq!(empty.rows[0].name, "empty");
}

#[test]
fn snapshots_compare_and_reject_incompatible_volumes() {
    let (directory, snapshot) = analysis_fixture();
    let data = tempfile::tempdir().expect("data");
    let service = SnapshotService::new(SnapshotService::default_dir(data.path()));

    let base = service
        .create(
            &snapshot,
            &SnapshotCreateRequest {
                path: None,
                out: None,
            },
        )
        .expect("base");
    fs::write(directory.path().join("added.bin"), [7u8; 256]).expect("added");
    let target_snapshot = scanned(directory.path(), 12);
    let target = service
        .create(
            &target_snapshot,
            &SnapshotCreateRequest {
                path: None,
                out: None,
            },
        )
        .expect("target");

    let list = service.list(None).expect("list");
    assert_eq!(list.len(), 2);

    let task = service
        .compare(&base.snapshot_id, &target.snapshot_id)
        .expect("compare");
    let page = service
        .diff_page(
            &task,
            SnapshotDiffSort {
                key: SnapshotDiffSortKey::SizeDelta,
                descending: true,
            },
            10,
            None,
        )
        .expect("diff page");
    assert_eq!(page.added, 1);
    assert!(page.rows.iter().any(|row| row.path.contains("added.bin")));

    let other_dir = tempfile::tempdir().expect("other");
    let other = scanned(other_dir.path(), 13);
    let foreign = service
        .create(
            &other,
            &SnapshotCreateRequest {
                path: None,
                out: None,
            },
        )
        .expect("foreign snapshot");
    // Both temporary directories live on the same volume, so rewrite the
    // volume identifier to model a snapshot from another disk.
    let foreign_path =
        std::path::PathBuf::from(service.summary(&foreign.snapshot_id).expect("summary").path);
    let mut payload = freespace_report::SnapshotPayload::read(&foreign_path).expect("read");
    payload.volume.id = uuid::Uuid::new_v4().to_string();
    payload.checksum = payload.compute_checksum().expect("checksum");
    payload.write(&foreign_path).expect("rewrite");
    let error = service
        .compare(&base.snapshot_id, &foreign.snapshot_id)
        .expect_err("different volume");
    assert_eq!(error.code, freespace_app::error::UiErrorCode::Unsupported);

    service
        .delete(&target.snapshot_id)
        .expect("delete target snapshot");
    let error = service
        .diff_page(&task, SnapshotDiffSort::default(), 10, None)
        .expect_err("cached comparison is gone");
    assert_eq!(error.code, freespace_app::error::UiErrorCode::NotFound);
}
