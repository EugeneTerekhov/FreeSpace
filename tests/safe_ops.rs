//! US9: preview, deletion modes, journal and rename undo.

mod common;

use std::path::PathBuf;

use common::TestTree;
use freespace_core::ErrorCode;
use freespace_ops::{
    DeleteMode, DeleteOptions, Store,
    confirm::{is_protected_path, preview_delete, require_confirmation},
    delete::delete_paths,
    guarded::{enforce, inspect},
    operation::TargetStatus,
    undo::undo_operation,
};

fn setup() -> (TestTree, Store) {
    (TestTree::new(), Store::open_in_memory().expect("store"))
}

#[test]
fn builds_a_preview_with_the_available_deletion_modes() {
    let fixture = TestTree::new();
    let first = fixture.write("a.txt", &[1u8; 10]);
    let second = fixture.write("b.txt", &[2u8; 20]);

    let preview = preview_delete(&[first, second], Some(1_000)).expect("preview");

    assert_eq!(preview.total_count, 2);
    assert_eq!(preview.total_bytes, 30);
    assert_eq!(preview.delete_options.len(), 2);
    assert!(
        preview
            .delete_options
            .iter()
            .all(|option| option.mode != DeleteMode::RecycleBin || option.available)
    );
    assert!(preview.insufficient_space.is_none());
    assert!(
        !preview
            .warnings
            .iter()
            .any(|warning| warning.contains("missing"))
    );
}

#[test]
fn refuses_without_confirmation_and_keeps_data() {
    let fixture = TestTree::new();
    let path = fixture.write("keep.txt", b"payload");
    let store = Store::open_in_memory().expect("store");

    let error = delete_paths(
        &store,
        std::slice::from_ref(&path),
        &DeleteOptions::new(DeleteMode::Permanent),
        false,
    )
    .expect_err("must refuse");

    assert_eq!(error.code(), ErrorCode::ConfirmationRequired);
    assert_eq!(std::fs::read(&path).expect("read"), b"payload");
    assert!(require_confirmation(false).is_err());
}

#[test]
fn permanent_deletion_is_journalled_and_not_undoable() {
    let (fixture, store) = setup();
    let path = fixture.write("gone.bin", &[7u8; 64]);

    let outcome = delete_paths(
        &store,
        std::slice::from_ref(&path),
        &DeleteOptions::new(DeleteMode::Permanent),
        true,
    )
    .expect("delete");

    assert!(!path.exists());
    assert!(!outcome.undoable);
    assert_eq!(
        store.journal_list(1).expect("journal")[0].result.as_str(),
        "success"
    );
}

#[test]
fn reverses_journalled_renames() {
    let fixture = TestTree::new();
    let original = fixture.write("old.txt", b"payload");
    let renamed = fixture.path("new.txt");
    std::fs::rename(&original, &renamed).expect("rename");

    let store = Store::open_in_memory().expect("store");
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
            "renames": [{
                "from": original.to_string_lossy(),
                "to": renamed.to_string_lossy(),
            }],
        }),
    };
    store.journal_append(&entry).expect("append");

    let undone = undo_operation(&store, entry.operation_id).expect("undo");

    assert_eq!(undone.restored, 1);
    assert_eq!(std::fs::read(&original).expect("read"), b"payload");
    assert!(!renamed.exists());
    assert!(!store.journal_list(1).expect("journal")[0].undo_available);
}

#[cfg(windows)]
fn protected_probe() -> (PathBuf, PathBuf) {
    let root =
        PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into()));
    let path = root.join("System32").join("kernel32.dll");

    (root, path)
}

#[cfg(unix)]
fn protected_probe() -> (PathBuf, PathBuf) {
    (PathBuf::from("/etc"), PathBuf::from("/etc/passwd"))
}

#[test]
fn blocks_paths_outside_the_scope_and_protected_paths() {
    let fixture = TestTree::new();
    let inside = fixture.write("inside.txt", b"x");
    let outside = fixture
        .root()
        .parent()
        .expect("temp parent")
        .join("outside.txt");
    let (protected_root, protected) = protected_probe();

    let report = inspect(
        fixture.root(),
        &[inside.clone(), outside.clone(), protected.clone()],
    );
    assert!(report.outside_scope.contains(&outside));
    assert_eq!(report.protected.len(), 1);
    assert!(report.requires_extra_confirmation);

    enforce(fixture.root(), &[inside], false).expect("inside the scope");
    let error = enforce(fixture.root(), &[outside], false).expect_err("outside");
    assert_eq!(error.code(), ErrorCode::Operation);
    let error = enforce(&protected_root, &[protected], false).expect_err("protected");
    assert_eq!(error.code(), ErrorCode::ProtectedPath);
    assert!(is_protected_path(&protected_root));
}

#[test]
fn reports_per_object_failures_when_one_target_is_missing() {
    let (fixture, store) = setup();
    let present = fixture.write("present.txt", b"payload");
    let missing = fixture.path("missing.txt");

    let outcome = delete_paths(
        &store,
        &[present.clone(), missing],
        &DeleteOptions::new(DeleteMode::Permanent),
        true,
    )
    .expect("the present object is still processed");

    assert_eq!(outcome.summary.succeeded, 1);
    assert_eq!(outcome.summary.failed, 1);
    assert!(outcome.first_error().is_some());
    assert!(!present.exists());
}

#[test]
fn records_result_summaries_in_the_journal() {
    let (fixture, store) = setup();
    let path = fixture.write("report.bin", &[0u8; 32]);

    let outcome = delete_paths(
        &store,
        &[path],
        &DeleteOptions::new(DeleteMode::Permanent),
        true,
    )
    .expect("delete");

    assert_eq!(outcome.summary.succeeded, 1);
    assert_eq!(outcome.summary.failed, 0);
    assert_eq!(outcome.items[0].error, None);
    assert!(!outcome.undoable);
    let entry = &store.journal_list(1).expect("journal")[0];
    assert_eq!(entry.affected_count, 1);
    assert_eq!(entry.affected_bytes, 32);
    assert_eq!(entry.details["kind"], "delete");
    assert!(entry.details["targets"].as_array().is_some());
    let _ = TargetStatus::Succeeded;
}
