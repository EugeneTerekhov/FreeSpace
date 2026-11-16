//! US9: preview, deletion modes, journal, quarantine restore and undo.

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
    quarantine::Quarantine,
    undo::undo_operation,
};

fn setup() -> (TestTree, tempfile::TempDir, Store) {
    (
        TestTree::new(),
        tempfile::tempdir().expect("quarantine"),
        Store::open_in_memory().expect("store"),
    )
}

#[test]
fn builds_a_preview_with_all_deletion_modes() {
    let fixture = TestTree::new();
    let first = fixture.write("a.txt", &[1u8; 10]);
    let second = fixture.write("b.txt", &[2u8; 20]);

    let preview = preview_delete(&[first, second], Some(1_000)).expect("preview");

    assert_eq!(preview.total_count, 2);
    assert_eq!(preview.total_bytes, 30);
    assert_eq!(preview.delete_options.len(), 3);
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
        None,
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
fn quarantines_journals_and_restores_objects() {
    let (fixture, root, store) = setup();
    let first = fixture.write("first.txt", b"one");
    let second = fixture.write("second.txt", b"two");
    let quarantine = Quarantine::new(root.path().join("store")).expect("quarantine");

    let outcome = delete_paths(
        &store,
        Some(&quarantine),
        &[first.clone(), second.clone()],
        &DeleteOptions::new(DeleteMode::Quarantine),
        true,
    )
    .expect("delete");

    assert_eq!(outcome.summary.succeeded, 2);
    assert_eq!(store.journal_list(10).expect("journal").len(), 1);
    assert!(!first.exists());

    let entry = store.journal_list(1).expect("journal").remove(0);
    assert!(entry.undo_available);
    let undone = undo_operation(&store, &quarantine, entry.operation_id).expect("undo");

    assert_eq!(undone.restored, 2);
    assert_eq!(std::fs::read(&first).expect("read"), b"one");
    assert_eq!(std::fs::read(&second).expect("read"), b"two");
    assert!(!store.journal_list(1).expect("journal")[0].undo_available);
}

#[test]
fn permanent_deletion_is_not_undoable() {
    let (fixture, root, store) = setup();
    let path = fixture.write("gone.bin", &[7u8; 64]);
    let quarantine = Quarantine::new(root.path().join("store")).expect("quarantine");

    let outcome = delete_paths(
        &store,
        Some(&quarantine),
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
fn blocks_paths_outside_the_scope_and_protected_paths() {
    let fixture = TestTree::new();
    let inside = fixture.write("inside.txt", b"x");
    let outside = fixture.root().join("..").join("outside.txt");
    let system_root =
        PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into()));
    let protected = system_root.join("System32").join("kernel32.dll");

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
    let error = enforce(&system_root, &[protected], false).expect_err("protected");
    assert_eq!(error.code(), ErrorCode::ProtectedPath);
    assert!(is_protected_path(&system_root.join("System32")));
}

#[test]
fn reports_per_object_failures_when_one_target_is_missing() {
    let (fixture, root, store) = setup();
    let present = fixture.write("present.txt", b"payload");
    let missing = fixture.path("missing.txt");
    let quarantine = Quarantine::new(root.path().join("store")).expect("quarantine");

    let outcome = delete_paths(
        &store,
        Some(&quarantine),
        &[present.clone(), missing],
        &DeleteOptions::new(DeleteMode::Quarantine),
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
    let (fixture, root, store) = setup();
    let path = fixture.write("report.bin", &[0u8; 32]);
    let quarantine = Quarantine::new(root.path().join("store")).expect("quarantine");

    let outcome = delete_paths(
        &store,
        Some(&quarantine),
        &[path],
        &DeleteOptions::new(DeleteMode::Quarantine),
        true,
    )
    .expect("delete");

    assert_eq!(outcome.summary.succeeded, 1);
    assert_eq!(outcome.summary.failed, 0);
    assert_eq!(outcome.items[0].error, None);
    assert_eq!(outcome.items[0].quarantine_id.is_some(), outcome.undoable);
    let entry = &store.journal_list(1).expect("journal")[0];
    assert_eq!(entry.affected_count, 1);
    assert_eq!(entry.affected_bytes, 32);
    assert_eq!(entry.details["kind"], "delete");
    assert!(entry.details["targets"].as_array().is_some());
    let _ = TargetStatus::Succeeded;
}
