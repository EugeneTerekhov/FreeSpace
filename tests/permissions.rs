//! US13: batch attributes, ownership and NTFS permissions.
//!
//! Permission and ownership changes are administrative tests: they only run when
//! `FREESPACE_ADMIN_TESTS=1` is set.

mod common;

use common::TestTree;
use freespace_ops::{
    AclAction, AttributeChange, HIDDEN, Store, acl::apply_acl, attributes::apply_attributes,
};
use freespace_platform::security::{copy_acl, get_attributes, owner};

#[test]
fn changes_attributes_and_reports_every_object() {
    let fixture = TestTree::new();
    let first = fixture.write("first.txt", b"one");
    let second = fixture.write("second.txt", b"two");
    let store = Store::open_in_memory().expect("store");

    let report = apply_attributes(
        Some(&store),
        &[first.clone(), second.clone()],
        AttributeChange {
            set: HIDDEN,
            clear: 0,
        },
    )
    .expect("attributes");

    assert_eq!(report.succeeded(), 2);
    assert_eq!(report.failed(), 0);
    assert!(report.journal_id.is_some());
    assert_eq!(get_attributes(&first).expect("attributes") & HIDDEN, HIDDEN);
    assert_eq!(
        get_attributes(&second).expect("attributes") & HIDDEN,
        HIDDEN
    );

    apply_attributes(
        Some(&store),
        &[first, second],
        AttributeChange {
            set: 0,
            clear: HIDDEN,
        },
    )
    .expect("clear");
}

#[test]
fn reports_unreadable_objects_without_stopping_the_batch() {
    let fixture = TestTree::new();
    let present = fixture.write("present.txt", b"payload");
    let missing = fixture.path("missing.txt");

    let report = apply_attributes(
        None,
        &[present, missing],
        AttributeChange {
            set: HIDDEN,
            clear: 0,
        },
    )
    .expect("report");

    assert_eq!(report.succeeded(), 1);
    assert_eq!(report.failed(), 1);
    assert!(report.items.iter().any(|item| item.error.is_some()));
}

#[test]
fn changes_owner_and_permissions_when_administrative_tests_are_enabled() {
    let Some(_guard) = common::require_admin() else {
        return;
    };
    let fixture = TestTree::new();
    let path = fixture.write("owned.txt", b"payload");
    let store = Store::open_in_memory().expect("store");
    let user = std::env::var("USERNAME").expect("USERNAME");
    let domain = std::env::var("USERDOMAIN").unwrap_or_else(|_| ".".to_owned());
    let account = format!("{domain}\\{user}");

    let report = apply_acl(
        Some(&store),
        std::slice::from_ref(&path),
        &AclAction::SetOwner(account.clone()),
    )
    .expect("owner");

    assert_eq!(report.failed(), 0, "report: {report:?}");
    assert!(owner(&path).expect("owner").is_some());

    apply_acl(Some(&store), std::slice::from_ref(&path), &AclAction::Reset).expect("reset");

    let source = fixture.write("source.txt", b"source");
    copy_acl(&source, &path).expect("copy acl");
}
