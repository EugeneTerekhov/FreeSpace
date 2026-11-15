//! US1 fallback directory-walk tests.
//!
//! The fixture lives in a temporary directory; no real user data is touched.

mod common;

use common::TestTree;
use freespace_core::{
    model::{ScanMethod, ScanStatus},
    query::{FilterSpec, SortDirection, SortKey, SortSpec, filtered_children, search},
    tree::NodeKind,
};
use freespace_scan::{
    MethodChoice, NoopObserver, ScanRequest,
    session::{ScanObserver, scan},
};

#[test]
fn walks_a_fixture_and_aggregates_sizes() {
    let fixture = TestTree::sample();
    let request = ScanRequest {
        method: MethodChoice::Walk,
        ..ScanRequest::new(fixture.root())
    };

    let result = scan(&request, &mut NoopObserver).expect("scan");
    let root = result.tree.roots().next().expect("root");
    let expected = fixture.total_file_bytes();

    assert_eq!(result.method, ScanMethod::DirectoryWalk);
    assert_eq!(result.session.status, ScanStatus::Completed);
    assert_eq!(result.tree.node(root).size.logical, expected);
    assert_eq!(result.tree.node(root).file_count(), 5);
    assert_eq!(result.tree.node(root).dir_count(), 5);
    assert!(result.tree.node(root).size.logical > 0);
    result.tree.validate().expect("consistent tree");
    assert!(!result.session.is_running());
}

#[test]
fn preserves_unicode_names() {
    let fixture = TestTree::sample();
    let request = ScanRequest {
        method: MethodChoice::Walk,
        ..ScanRequest::new(fixture.root())
    };
    let result = scan(&request, &mut NoopObserver).expect("scan");

    let root = result.tree.roots().next().expect("root");
    let unicode = result
        .tree
        .children(root)
        .find(|id| result.tree.name(*id) == "UNICODE-имя")
        .expect("unicode directory");

    assert_eq!(result.tree.node(unicode).kind, NodeKind::Directory);
    assert_eq!(result.tree.node(unicode).file_count(), 1);
}

#[test]
fn applies_exclusions_and_reports_them() {
    let fixture = TestTree::sample();
    let request = ScanRequest {
        method: MethodChoice::Walk,
        exclusions: vec!["beta".to_owned()],
        ..ScanRequest::new(fixture.root())
    };

    let result = scan(&request, &mut NoopObserver).expect("scan");
    let root = result.tree.roots().next().expect("root");

    assert!(result.excluded > 0);
    assert!(
        result
            .warnings
            .iter()
            .any(|warning| warning.contains("excluded"))
    );
    assert!(
        result
            .tree
            .children(root)
            .all(|id| result.tree.name(id) != "beta")
    );
    assert_eq!(
        result.tree.node(root).size.logical,
        fixture.total_file_bytes() - 100
    );
}

#[test]
fn supports_search_and_sorting_after_a_scan() {
    let fixture = TestTree::sample();
    let request = ScanRequest {
        method: MethodChoice::Walk,
        ..ScanRequest::new(fixture.root())
    };
    let result = scan(&request, &mut NoopObserver).expect("scan");
    let root = result.tree.roots().next().expect("root");

    let by_size = filtered_children(
        &result.tree,
        root,
        &FilterSpec {
            kinds: vec![NodeKind::Directory],
            ..FilterSpec::default()
        }
        .compile()
        .expect("filter"),
        SortSpec::new(SortKey::Size, SortDirection::Descending),
    );
    assert!(!by_size.is_empty());
    assert!(
        result.tree.node(by_size[0]).size.logical
            >= result.tree.node(by_size[by_size.len() - 1]).size.logical
    );

    let filter = FilterSpec {
        pattern: Some("*.txt".to_owned()),
        ..FilterSpec::default()
    }
    .compile()
    .expect("filter");
    let matches = search(
        &result.tree,
        root,
        &filter,
        SortSpec::new(SortKey::Name, SortDirection::Ascending),
        true,
        100,
    );

    assert!(
        matches
            .iter()
            .any(|id| result.tree.name(*id) == "first.txt")
    );
    assert!(
        matches
            .iter()
            .any(|id| result.tree.name(*id) == "second.txt")
    );
    assert!(matches.iter().any(|id| result.tree.name(*id) == "файл.txt"));
}

#[test]
fn cancellation_keeps_the_partial_tree() {
    struct Cancelling;

    impl ScanObserver for Cancelling {
        fn is_cancelled(&self) -> bool {
            true
        }
    }

    let fixture = TestTree::sample();
    let request = ScanRequest::new(fixture.root());

    let result = scan(&request, &mut Cancelling).expect("partial scan");

    assert!(result.cancelled);
    assert_eq!(result.session.status, ScanStatus::Cancelled);
    assert_eq!(result.tree.live_len(), 1);
}
