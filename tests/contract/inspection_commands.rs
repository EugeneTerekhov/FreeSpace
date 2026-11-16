//! Contract checks for the inspection command family (US5, T108).
//!
//! The Tauri adapters are thin; these tests drive the same `freespace-app`
//! services against temporary directories and assert the documented invariants.
//! Everything lives under `tempfile`; administrative actions stay disabled
//! unless `FREESPACE_ADMIN_TESTS=1` is set explicitly.

#[path = "../common/mod.rs"]
mod common;

use std::{
    fs,
    io::Write as _,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use freespace_app::{
    archives,
    dto::{
        ExportComposition, ExportFormat, ExportPreviewRequest, ExportScope, ExportSection,
        FilterSet, Metric, ScanMethodChoice, ScanRequest, SortSpec, TaskState, UnlockMode,
    },
    error::UiErrorCode,
    export::{self, ExportService},
    metadata,
    owners::{OwnerResolver, OwnerService},
    scan::run_scan,
    tree::TreeSnapshot,
    unlock,
};
use freespace_ops::Store;
use freespace_platform::elevation;
use freespace_platform::security::OwnerIdentity;
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

fn fixture() -> (tempfile::TempDir, TreeSnapshot) {
    let directory = tempfile::tempdir().expect("temp dir");
    fs::write(directory.path().join("notes.txt"), b"plain text").expect("notes");
    fs::write(
        directory.path().join("broken.jpg"),
        b"\xFF\xD8\xFF\xE0not really a JPEG",
    )
    .expect("image");
    fs::write(directory.path().join("alice.bin"), [1u8; 100]).expect("alice");
    fs::write(directory.path().join("bob.bin"), [2u8; 250]).expect("bob");
    fs::write(directory.path().join("shared.bin"), [3u8; 50]).expect("shared");
    let snapshot = scanned(directory.path(), 31);

    (directory, snapshot)
}

fn node_id(snapshot: &TreeSnapshot, name: &str) -> u32 {
    snapshot
        .search_page(
            name,
            Metric::Logical,
            SortSpec::default(),
            &FilterSet::default(),
            10,
            None,
        )
        .expect("search")
        .rows
        .first()
        .unwrap_or_else(|| panic!("node `{name}`"))
        .id
}

fn owner_resolver() -> OwnerResolver {
    ArcOwnerResolver::boxed(|path: &Path| {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let identity = match name {
            "alice.bin" => Some(("S-1-5-21-1", r"PC\Alice")),
            "bob.bin" => Some(("S-1-5-21-2", r"PC\Bob")),
            _ => None,
        };

        Ok(identity.map(|(sid, display_name)| OwnerIdentity {
            sid: sid.to_owned(),
            display_name: display_name.to_owned(),
        }))
    })
}

/// Small alias so the resolver closure type stays readable.
struct ArcOwnerResolver;

impl ArcOwnerResolver {
    fn boxed<F>(resolver: F) -> OwnerResolver
    where
        F: Fn(&Path) -> freespace_core::Result<Option<OwnerIdentity>> + Send + Sync + 'static,
    {
        std::sync::Arc::new(resolver)
    }
}

fn zip_fixture(directory: &Path) -> PathBuf {
    let path = directory.join("sample.zip");
    let file = fs::File::create(&path).expect("create zip");
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    for (name, bytes) in [
        ("docs/readme.txt", b"hello".as_slice()),
        ("payload.bin", &[0u8; 128]),
        ("image.jpg", &[7u8; 64]),
    ] {
        writer.start_file(name, options).expect("entry");
        writer.write_all(bytes).expect("payload");
    }
    writer.finish().expect("finish");

    path
}

fn wait_for_export(service: &ExportService, task_id: &str) -> freespace_app::dto::ExportStatus {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let status = service.status(task_id).expect("export status");
        if status.state.is_finished() {
            return status;
        }
        assert!(Instant::now() < deadline, "export did not finish");
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn metadata_reports_available_records_and_keeps_absence_quiet() {
    let (directory, _) = fixture();
    fs::copy(
        std::env::current_exe().expect("current executable"),
        directory.path().join("fixture.exe"),
    )
    .expect("copy executable");
    let snapshot = scanned(directory.path(), 32);

    let executable =
        metadata::metadata_view(&snapshot, node_id(&snapshot, "fixture.exe")).expect("PE metadata");
    assert_eq!(executable.kind, "pe");
    assert_eq!(executable.source_key, "metadata.source.pe");
    assert_eq!(executable.status, "ok");
    assert!(executable.available);
    assert!(executable.size_bytes > 0);

    let unsupported = metadata::metadata_view(&snapshot, node_id(&snapshot, "notes.txt"))
        .expect_err("plain text has no metadata");
    assert_eq!(unsupported.code, UiErrorCode::Unsupported);
    assert!(
        unsupported
            .details
            .as_deref()
            .is_some_and(|details| details.contains("metadata.unsupported.extension"))
    );

    let corrupted = metadata::metadata_view(&snapshot, node_id(&snapshot, "broken.jpg"))
        .expect_err("damaged image");
    assert_eq!(corrupted.code, UiErrorCode::Unsupported);
    assert!(
        corrupted
            .details
            .as_deref()
            .is_some_and(|details| details.contains("metadata.corrupt"))
    );

    let stale = metadata::metadata_view(&snapshot, 999_999).expect_err("stale node");
    assert_eq!(stale.code, UiErrorCode::StaleNode);
}

#[test]
fn owners_pages_summaries_and_files_with_generation_bound_cursors() {
    let (directory, snapshot) = fixture();
    let service = OwnerService::with_resolver(owner_resolver());

    let first = service
        .owners_page(&snapshot, &FilterSet::default(), 2, None)
        .expect("owner page");
    assert_eq!(first.generation, 31);
    assert_eq!(first.total_bytes, 431);
    assert_eq!(first.unknown_bytes, 81);
    assert_eq!(first.rows.len(), 2);
    assert_eq!(first.rows[0].display_name, r"PC\Bob");
    assert!(first.rows.iter().all(|row| row.accessible));
    assert!(first.next_cursor.is_some());

    let second = service
        .owners_page(
            &snapshot,
            &FilterSet::default(),
            2,
            first.next_cursor.as_deref(),
        )
        .expect("second owner page");
    assert_eq!(second.rows.len(), 1);
    assert_eq!(second.rows[0].display_name, "unknown");
    assert_eq!(second.rows[0].label_key, "owners.label.unknown");

    let filtered = FilterSet {
        text: "alice".to_owned(),
        ..FilterSet::default()
    };
    let stale = service
        .owners_page(&snapshot, &filtered, 2, first.next_cursor.as_deref())
        .expect_err("cursor belongs to another filter");
    assert_eq!(stale.code, UiErrorCode::StaleCursor);

    let other_generation = scanned(directory.path(), 32);
    let stale_generation = service
        .owners_page(
            &other_generation,
            &FilterSet::default(),
            2,
            first.next_cursor.as_deref(),
        )
        .expect_err("cursor belongs to another generation");
    assert_eq!(stale_generation.code, UiErrorCode::StaleCursor);

    let files = service
        .owners_files_page(
            &snapshot,
            r"PC\Alice",
            &FilterSet::default(),
            Metric::Logical,
            SortSpec::default(),
            10,
            None,
        )
        .expect("alice files");
    assert_eq!(files.generation, 31);
    assert_eq!(files.rows.len(), 1);
    assert_eq!(files.rows[0].name, "alice.bin");

    let unknown = service
        .owners_files_page(
            &snapshot,
            "unknown",
            &FilterSet::default(),
            Metric::Logical,
            SortSpec::default(),
            10,
            None,
        )
        .expect("unknown files");
    assert_eq!(unknown.rows.len(), 3);
    assert!(unknown.rows.iter().any(|row| row.name == "shared.bin"));

    let stale_files = service
        .owners_files_page(
            &snapshot,
            r"PC\Alice",
            &filtered,
            Metric::Logical,
            SortSpec::default(),
            10,
            None,
        )
        .expect("filtered files");
    assert_eq!(stale_files.rows.len(), 1);
}

#[test]
fn archive_listing_never_extracts_and_reports_unsupported_containers() {
    let directory = tempfile::tempdir().expect("temp dir");
    let archive = zip_fixture(directory.path());

    let first =
        archives::entries_page(&archive, &FilterSet::default(), 1, None).expect("archive page");
    assert_eq!(first.format, "zip");
    assert_eq!(first.total_entries, 3);
    assert_eq!(first.total_files, 3);
    assert_eq!(first.entries.len(), 1);
    assert_eq!(first.entries[0].internal_path, "payload.bin");
    assert!(first.next_cursor.is_some());

    let second = archives::entries_page(
        &archive,
        &FilterSet::default(),
        5,
        first.next_cursor.as_deref(),
    )
    .expect("next page");
    assert_eq!(second.entries.len(), 2);
    assert!(second.next_cursor.is_none());

    let filtered = FilterSet {
        text: "readme".to_owned(),
        ..FilterSet::default()
    };
    let single = archives::entries_page(&archive, &filtered, 5, None).expect("filtered page");
    assert_eq!(single.entries.len(), 1);
    assert_eq!(single.entries[0].internal_path, "docs/readme.txt");
    assert_eq!(single.entries[0].depth, 1);

    let stale = archives::entries_page(&archive, &filtered, 1, first.next_cursor.as_deref())
        .expect_err("cursor from another filter");
    assert_eq!(stale.code, UiErrorCode::StaleCursor);

    let other_directory = tempfile::tempdir().expect("other temp dir");
    let other_archive = zip_fixture(other_directory.path());
    let mut bytes = fs::read(&other_archive).expect("read other archive");
    bytes.push(0);
    fs::write(&other_archive, bytes).expect("rewrite other archive");
    let stale_archive = archives::entries_page(
        &other_archive,
        &FilterSet::default(),
        1,
        first.next_cursor.as_deref(),
    )
    .expect_err("cursor from another archive");
    assert_eq!(stale_archive.code, UiErrorCode::StaleCursor);

    let out = directory.path().join("archive-list.csv");
    let exported = archives::export_list(&archive, ExportFormat::Csv, &out).expect("list export");
    assert_eq!(exported.rows_written, 3);
    assert!(exported.bytes_written > 0);
    let text = fs::read_to_string(&out).expect("list file");
    assert!(text.contains("payload.bin"));
    assert!(
        !directory.path().join("payload.bin").exists(),
        "no extraction"
    );

    let bad = directory.path().join("broken.zip");
    fs::write(&bad, b"PK\x03\x04this is not a zip").expect("broken zip");
    let unsupported =
        archives::entries_page(&bad, &FilterSet::default(), 10, None).expect_err("damaged");
    assert_eq!(unsupported.code, UiErrorCode::Unsupported);
    assert!(
        unsupported
            .details
            .as_deref()
            .is_some_and(|details| details.contains("archive.unsupported."))
    );

    let missing = archives::entries_page(
        &directory.path().join("missing.zip"),
        &FilterSet::default(),
        10,
        None,
    )
    .expect_err("missing archive");
    assert_eq!(missing.code, UiErrorCode::NotFound);
}

#[test]
fn export_preview_token_expiry_overwrite_and_cancellation() {
    let (directory, snapshot) = fixture();
    let output = tempfile::tempdir().expect("output");
    let snapshot = std::sync::Arc::new(snapshot);
    let service = ExportService::new();

    let out = output.path().join("report.csv");
    let request = ExportPreviewRequest {
        scope: ExportScope {
            generation: 31,
            out: out.to_string_lossy().into_owned(),
        },
        format: ExportFormat::Csv,
        composition: ExportComposition {
            sections: vec![ExportSection::Tree, ExportSection::Top],
            max_tree_rows: None,
            overwrite: false,
        },
    };
    let preview = service
        .preview(&request, std::sync::Arc::clone(&snapshot), 0)
        .expect("preview");
    assert_eq!(preview.total_bytes, 431);
    assert!(preview.row_estimate.is_some());
    assert!(!preview.confirmation_token.is_empty());
    assert_eq!(service.preview_count(), 1);

    let mut stale_request = request.clone();
    stale_request.scope.generation = 30;
    let stale_generation = service
        .preview(&stale_request, std::sync::Arc::clone(&snapshot), 0)
        .expect_err("stale generation");
    assert_eq!(stale_generation.code, UiErrorCode::StaleCursor);

    let started = service
        .start(&preview.preview_id, &preview.confirmation_token, 1)
        .expect("start");
    let status = wait_for_export(&service, &started.task_id);
    assert_eq!(status.state, TaskState::Completed, "{status:?}");
    assert!(status.rows_written >= 7);
    assert!(out.exists());
    let text = fs::read_to_string(&out).expect("report");
    assert!(text.contains("alice.bin"));

    let reused = service
        .start(&preview.preview_id, &preview.confirmation_token, 2)
        .expect_err("preview is one-time");
    assert_eq!(reused.code, UiErrorCode::PreviewExpired);

    let existing = service
        .preview(&request, std::sync::Arc::clone(&snapshot), 3)
        .expect_err("existing file needs consent");
    assert_eq!(existing.code, UiErrorCode::Validation);

    let mut overwrite = request.clone();
    overwrite.composition.overwrite = true;
    let allowed = service
        .preview(&overwrite, std::sync::Arc::clone(&snapshot), 4)
        .expect("overwrite preview");
    assert!(allowed.overwrite);
    assert!(
        allowed
            .warnings
            .iter()
            .any(|warning| warning == "export.warning.overwrite")
    );

    let expiring = ExportService::with_ttl(1_000);
    let mut expiring_request = request.clone();
    expiring_request.scope.out = output
        .path()
        .join("expiring.csv")
        .to_string_lossy()
        .into_owned();
    let preview = expiring
        .preview(&expiring_request, std::sync::Arc::clone(&snapshot), 10)
        .expect("expiring preview");
    let expired = expiring
        .start(&preview.preview_id, &preview.confirmation_token, 1_011)
        .expect_err("expired");
    assert_eq!(expired.code, UiErrorCode::PreviewExpired);

    let slow = ExportService::with_options(60_000, Duration::from_millis(200));
    let cancelled_out = output.path().join("cancelled.csv");
    let mut cancelled_request = request.clone();
    cancelled_request.scope.out = cancelled_out.to_string_lossy().into_owned();
    let preview = slow
        .preview(&cancelled_request, snapshot, 20)
        .expect("cancel preview");
    let started = slow
        .start(&preview.preview_id, &preview.confirmation_token, 21)
        .expect("start");
    thread::sleep(Duration::from_millis(20));
    slow.cancel(&started.task_id).expect("cancel");
    let status = wait_for_export(&slow, &started.task_id);
    assert_eq!(status.state, TaskState::Cancelled, "{status:?}");
    assert!(!cancelled_out.exists());

    assert_eq!(
        service
            .status("unknown-task")
            .expect_err("unknown task")
            .code,
        UiErrorCode::NotFound
    );
    assert_eq!(
        export::status("unknown-task")
            .expect_err("unknown global task")
            .code,
        UiErrorCode::NotFound
    );

    let _ = directory;
}

#[test]
fn unlock_lists_locks_and_refuses_privileged_actions_cleanly() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("held.txt");
    fs::write(&path, b"payload").expect("file");

    let locks = unlock::locks(&path).expect("lock listing");
    assert!(locks.is_empty(), "an unused temporary file has no holders");

    let missing = unlock::locks(&directory.path().join("missing.txt")).expect_err("missing");
    assert_eq!(missing.code, UiErrorCode::NotFound);

    let store = Store::open_in_memory().expect("store");
    let refused = unlock::release(&path, UnlockMode::Soft, &[], false, false, Some(&store))
        .expect_err("confirmation");
    assert_eq!(refused.code, UiErrorCode::Validation);

    let soft = unlock::release(&path, UnlockMode::Soft, &[], true, false, Some(&store))
        .expect("soft release");
    assert_eq!(soft.mode, UnlockMode::Soft);
    assert!(soft.available);
    assert!(soft.journal_entry_id.is_some());
    assert_eq!(store.journal_list(5).expect("journal").len(), 1);

    let force = unlock::release(
        &path,
        UnlockMode::Force,
        &[std::process::id()],
        true,
        false,
        None,
    )
    .expect_err("force needs elevation");
    assert_eq!(force.code, UiErrorCode::ElevationRequired);

    let no_selection =
        unlock::release(&path, UnlockMode::Force, &[], true, true, None).expect_err("selection");
    assert_eq!(no_selection.code, UiErrorCode::Validation);

    let no_confirmation = unlock::schedule_reboot(&path, false, false, None).expect_err("confirm");
    assert_eq!(no_confirmation.code, UiErrorCode::Validation);

    let no_elevation = unlock::schedule_reboot(&path, true, false, None).expect_err("elevation");
    assert_eq!(no_elevation.code, UiErrorCode::ElevationRequired);
    assert!(path.exists());
}

/// Administrative apply is gated by `FREESPACE_ADMIN_TESTS=1` and elevation.
#[test]
fn unlock_reboot_registration_is_admin_gated() {
    let Some(_guard) = common::require_admin() else {
        return;
    };
    if !elevation::is_elevated() {
        return;
    }

    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("reboot.txt");
    fs::write(&path, b"payload").expect("file");
    let store = Store::open_in_memory().expect("store");

    let result = unlock::schedule_reboot(&path, true, true, Some(&store)).expect("schedule reboot");
    assert!(result.scheduled_for_reboot);
    assert!(result.journal_entry_id.is_some());
}
