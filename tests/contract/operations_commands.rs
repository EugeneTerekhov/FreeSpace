//! Contract checks for previews, confirmation, journal, undo and cancellation
//! (US3). Only temporary directories are touched.

use std::fs;

use freespace_app::{
    dto::{DeleteMode, OperationKind, OperationPreviewRequest, OperationState},
    error::UiErrorCode,
    operations::{OperationService, execute_delete, journal_page, undo},
};
use freespace_ops::Store;

fn request(targets: &[&std::path::Path], kind: OperationKind) -> OperationPreviewRequest {
    OperationPreviewRequest {
        kind,
        targets: targets
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect(),
        delete_mode: None,
        reason: Some("contract".to_owned()),
        passes: None,
        task_id: None,
    }
}

fn fixture() -> (tempfile::TempDir, Store) {
    let workspace = tempfile::tempdir().expect("workspace");
    let store = Store::open_in_memory().expect("store");

    (workspace, store)
}

#[test]
fn preview_token_is_one_time_and_expires() {
    let (workspace, _store) = fixture();
    let path = workspace.path().join("victim.txt");
    fs::write(&path, b"payload").expect("write");
    let service = OperationService::with_ttl(1_000);
    let preview = service
        .preview(&request(&[&path], OperationKind::Delete), Some(1_000), 0)
        .expect("preview");

    assert_eq!(preview.total_size, 7);
    assert_eq!(preview.delete_mode, Some(DeleteMode::RecycleBin));
    assert!(preview.reversible);
    assert!(!preview.requires_elevation);

    service
        .take(
            &preview.preview_id,
            &preview.confirmation_token,
            None,
            false,
            false,
            500,
        )
        .expect("first take");

    let reused = service
        .take(
            &preview.preview_id,
            &preview.confirmation_token,
            None,
            false,
            false,
            600,
        )
        .expect_err("reuse");
    assert_eq!(reused.code, UiErrorCode::PreviewExpired);

    let preview = service
        .preview(&request(&[&path], OperationKind::Delete), None, 0)
        .expect("second preview");
    let expired = service
        .take(
            &preview.preview_id,
            &preview.confirmation_token,
            None,
            false,
            false,
            1_001,
        )
        .expect_err("expired");
    assert_eq!(expired.code, UiErrorCode::PreviewExpired);
}

#[test]
fn permanent_and_protected_operations_need_extra_confirmation() {
    let (workspace, _store) = fixture();
    let path = workspace.path().join("victim.txt");
    fs::write(&path, b"payload").expect("write");
    let service = OperationService::new();

    let mut permanent = request(&[&path], OperationKind::Delete);
    permanent.delete_mode = Some(DeleteMode::Permanent);
    let preview = service.preview(&permanent, None, 0).expect("preview");
    assert!(!preview.reversible);
    assert!(
        preview
            .warnings
            .iter()
            .any(|warning| warning == "confirm.warning.permanent")
    );
    let refused = service
        .take(
            &preview.preview_id,
            &preview.confirmation_token,
            Some(DeleteMode::Permanent),
            false,
            false,
            1,
        )
        .expect_err("needs extra confirmation");
    assert_eq!(refused.code, UiErrorCode::Validation);

    #[cfg(windows)]
    let protected = std::path::PathBuf::from(
        std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into()),
    )
    .join("System32")
    .join("kernel32.dll");
    #[cfg(unix)]
    let protected = std::path::PathBuf::from("/etc/passwd");
    let preview = service
        .preview(&request(&[&protected], OperationKind::Delete), None, 0)
        .expect("protected preview");
    assert!(preview.requires_elevation);
    let refused = service
        .take(
            &preview.preview_id,
            &preview.confirmation_token,
            None,
            true,
            false,
            1,
        )
        .expect_err("elevation");
    assert_eq!(refused.code, UiErrorCode::ElevationRequired);
}

#[test]
fn permanent_delete_is_journalled_and_not_undoable() {
    let (workspace, store) = fixture();
    let first = workspace.path().join("first.txt");
    let second = workspace.path().join("second.txt");
    fs::write(&first, b"one").expect("first");
    fs::write(&second, b"two").expect("second");
    let service = OperationService::new();
    let mut preview_request = request(&[&first, &second], OperationKind::Delete);
    preview_request.delete_mode = Some(DeleteMode::Permanent);
    let preview = service.preview(&preview_request, None, 0).expect("preview");
    let prepared = service
        .take(
            &preview.preview_id,
            &preview.confirmation_token,
            Some(DeleteMode::Permanent),
            true,
            false,
            1,
        )
        .expect("take");

    let mut progress_count = 0;
    let outcome = execute_delete(&service, &store, &prepared, "volume-1", None, &mut |_| {
        progress_count += 1
    })
    .expect("execute");

    assert_eq!(outcome.state, OperationState::Completed);
    assert_eq!(outcome.processed, 2);
    assert!(!outcome.undo_available);
    assert!(outcome.journal_entry_id.is_some());
    assert_eq!(progress_count, 2);
    assert!(!first.exists());
    assert!(!second.exists());

    let page = journal_page(
        &store,
        &freespace_app::dto::JournalFilter::default(),
        10,
        None,
    )
    .expect("journal");
    assert_eq!(page.rows.len(), 1);
    assert_eq!(page.rows[0].operation_id, outcome.operation_id);
    assert!(!page.rows[0].reversible);
    assert_eq!(page.rows[0].kind, OperationKind::Delete);
}

#[test]
fn undo_reverses_only_recorded_renames() {
    let (workspace, store) = fixture();
    let original = workspace.path().join("old.txt");
    let renamed = workspace.path().join("new.txt");
    fs::write(&original, b"payload").expect("write");
    fs::rename(&original, &renamed).expect("rename");

    let entry = freespace_ops::JournalEntry {
        id: 0,
        operation_id: freespace_core::Uuid::new_v4(),
        timestamp: freespace_core::Timestamp::from_millis(1_700_000_000_000),
        user: "tester".to_owned(),
        summary: "rename: 1 succeeded".to_owned(),
        affected_count: 1,
        affected_bytes: 0,
        result: freespace_ops::JournalResult::Success,
        undo_available: true,
        details: serde_json::json!({
            "kind": "rename",
            "renames": [{
                "from": original.to_string_lossy(),
                "to": renamed.to_string_lossy(),
            }],
        }),
    };
    store.journal_append(&entry).expect("append");

    let page = journal_page(
        &store,
        &freespace_app::dto::JournalFilter::default(),
        10,
        None,
    )
    .expect("journal");
    assert_eq!(page.rows.len(), 1);
    assert!(page.rows[0].reversible);
    assert_eq!(page.rows[0].kind, OperationKind::Rename);

    let restored = undo(&store, &entry.operation_id.to_string()).expect("undo");
    assert_eq!(restored, 1);
    assert!(original.exists());
    assert!(!renamed.exists());
    assert!(
        !journal_page(
            &store,
            &freespace_app::dto::JournalFilter::default(),
            10,
            None
        )
        .expect("journal")
        .rows[0]
            .reversible
    );
}

#[test]
fn one_operation_per_volume_and_cooperative_cancel() {
    let (workspace, store) = fixture();
    let path = workspace.path().join("victim.txt");
    fs::write(&path, b"payload").expect("write");
    let service = OperationService::new();
    service.begin("operation-1", "volume-9", 1).expect("begin");
    let busy = service
        .begin("operation-2", "volume-9", 1)
        .expect_err("busy");
    assert_eq!(busy.code, UiErrorCode::Busy);

    let preview = service
        .preview(&request(&[&path], OperationKind::Delete), None, 0)
        .expect("preview");
    let prepared = service
        .take(
            &preview.preview_id,
            &preview.confirmation_token,
            None,
            false,
            false,
            1,
        )
        .expect("take");
    // `take` does not reserve the volume; `execute` does.
    service
        .cancel(&prepared.operation_id.to_string())
        .expect_err("not started");

    let mut preview_request = request(&[&path], OperationKind::Delete);
    preview_request.delete_mode = Some(DeleteMode::Permanent);
    let preview = service.preview(&preview_request, None, 0).expect("preview");
    let prepared = service
        .take(
            &preview.preview_id,
            &preview.confirmation_token,
            Some(DeleteMode::Permanent),
            true,
            true,
            1,
        )
        .expect("take");
    service
        .begin(&prepared.operation_id.to_string(), "volume-10", 1)
        .expect("begin");
    service
        .cancel(&prepared.operation_id.to_string())
        .expect("cancel");
    let flag = service
        .cancel_flag(&prepared.operation_id.to_string())
        .expect("flag");
    let outcome = freespace_ops::delete_paths_cancellable(
        &store,
        &prepared.targets,
        &freespace_ops::DeleteOptions::new(freespace_ops::DeleteMode::Permanent),
        true,
        prepared.operation_id,
        Some(flag.as_ref()),
        &mut |_, _, _| {},
    )
    .expect("cancel outcome");
    assert_eq!(outcome.status, freespace_ops::OperationStatus::Cancelled);
    assert!(
        path.exists(),
        "cancellation before the first target removes nothing"
    );
}

#[test]
fn destructive_work_never_starts_without_a_preview() {
    let (workspace, _store) = fixture();
    let path = workspace.path().join("victim.txt");
    fs::write(&path, b"payload").expect("write");
    let service = OperationService::new();
    let error = service
        .take("unknown-preview", "unknown-token", None, false, false, 0)
        .expect_err("no preview");
    assert_eq!(error.code, UiErrorCode::PreviewExpired);
    assert!(path.exists());

    let invalid = service
        .preview(&request(&[], OperationKind::Delete), None, 0)
        .expect_err("no targets");
    assert_eq!(invalid.code, UiErrorCode::Validation);
}
