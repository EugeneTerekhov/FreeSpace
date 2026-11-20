//! US12: bulk rename preview, conflicts, results and rollback.

mod common;

use common::TestTree;
use freespace_ops::{
    CaseTransform, RenameRule, Store,
    rename_plan::{apply_plan, build_plan},
    undo::undo_operation,
};

#[test]
fn previews_applies_and_rolls_back_a_thousand_files() {
    let fixture = TestTree::new();
    let mut paths = Vec::with_capacity(1_000);
    for index in 0..1_000 {
        paths.push(fixture.write(format!("file-{index:04}.txt"), b"payload"));
    }
    let rule = RenameRule::new(
        r"^file-(\d+)\.txt$",
        "renamed-$1.dat",
        CaseTransform::Preserve,
    )
    .expect("rule");

    let plan = build_plan(&paths, &rule).expect("plan");
    assert_eq!(plan.items.len(), 1_000);
    assert!(plan.is_clear());
    let store = Store::open_in_memory().expect("store");
    let outcome = apply_plan(&plan, Some(&store)).expect("apply");

    assert_eq!(outcome.renamed, 1_000);
    assert!(fixture.path("renamed-0000.dat").exists());
    assert!(!fixture.path("file-0000.txt").exists());
    assert_eq!(store.journal_list(10).expect("journal").len(), 1);

    let undone = undo_operation(&store, outcome.operation_id).expect("undo");

    assert_eq!(undone.restored, 1_000);
    assert!(fixture.path("file-0000.txt").exists());
    assert!(!fixture.path("renamed-0000.dat").exists());
}

#[test]
fn blocks_duplicate_targets_before_changing_anything() {
    let fixture = TestTree::new();
    let first = fixture.write("a.txt", b"a");
    let second = fixture.write("b.txt", b"b");
    let rule = RenameRule::new(r"^[ab]", "same", CaseTransform::Preserve).expect("rule");

    let plan = build_plan(&[first.clone(), second.clone()], &rule).expect("plan");

    assert!(!plan.is_clear());
    assert!(
        plan.conflicts
            .iter()
            .any(|conflict| conflict.reason.contains("same name"))
    );
    assert!(first.exists() && second.exists());
    assert!(apply_plan(&plan, None).is_err());
}

#[test]
fn blocks_an_existing_target() {
    let fixture = TestTree::new();
    let source = fixture.write("source.txt", b"one");
    fixture.write("target.txt", b"two");
    let rule = RenameRule::new(r"^source", "target", CaseTransform::Preserve).expect("rule");

    let plan = build_plan(&[source], &rule).expect("plan");

    assert!(
        plan.conflicts
            .iter()
            .any(|conflict| conflict.reason.contains("already exists"))
    );
}

#[test]
fn validates_case_transforms_and_invalid_names() {
    let fixture = TestTree::new();
    let path = fixture.write("Photo.JPEG", b"payload");
    let rule = RenameRule::new(r"\.JPEG$", ".jpg", CaseTransform::Lower).expect("rule");
    let plan = build_plan(std::slice::from_ref(&path), &rule).expect("plan");

    assert_eq!(plan.items[0].new_name, "photo.jpg");
    let forbidden = RenameRule::new(r"^Photo", "a:b", CaseTransform::Preserve).expect("rule");
    let plan = build_plan(&[path], &forbidden).expect("plan");
    assert!(
        !plan.is_clear(),
        "a forbidden character must be reported as a conflict"
    );
}
