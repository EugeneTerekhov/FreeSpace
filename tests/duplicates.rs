//! US5: duplicate detection on a real scanned tree.
//!
//! The fixture contains genuine duplicates and two files with the same size but
//! different contents; the pipeline must never confuse them (FR-026).

mod common;

use common::TestTree;
use freespace_analysis::{
    DuplicateOptions, HashAlgorithm, HashCache, KeepRule, apply_keep_rule, find_duplicates,
};
use freespace_scan::{MethodChoice, NoopObserver, ScanRequest, ScanResult, scan};

fn scan_tree(fixture: &TestTree) -> ScanResult {
    let request = ScanRequest {
        root: fixture.root().to_path_buf(),
        method: MethodChoice::Walk,
        exclusions: Vec::new(),
        incremental: false,
    };

    scan(&request, &mut NoopObserver).expect("scan fixture")
}

fn fixture() -> (TestTree, Vec<u8>) {
    let fixture = TestTree::new();
    let payload: Vec<u8> = (0..8_192).map(|index| (index % 251) as u8).collect();
    fixture.write("first/copy.bin", &payload);
    fixture.write("first/keep.bin", &payload);
    fixture.write("second/other.bin", &vec![0u8; payload.len()]);
    fixture.write("second/unique.bin", b"unique content");

    (fixture, payload)
}

#[test]
fn finds_only_byte_identical_files() {
    let (fixture, payload) = fixture();
    let result = scan_tree(&fixture);
    let cache = HashCache::open_in_memory().expect("cache");

    let search = find_duplicates(
        &result.tree,
        DuplicateOptions {
            min_size: 1,
            algorithm: HashAlgorithm::Sha256,
        },
        Some(&cache),
    )
    .expect("search");

    assert_eq!(search.groups.len(), 1, "equal sizes are not duplicates");
    let group = &search.groups[0];
    assert_eq!(group.members.len(), 2);
    assert_eq!(group.size, u64::try_from(payload.len()).expect("size"));
    assert_eq!(group.reclaimable_bytes, group.size);
    assert!(group.members.iter().all(|member| member.verified));
    assert!(!group.stale);
    assert_eq!(group.selected_count(), 1);
    assert!(
        group
            .members
            .iter()
            .any(|member| !member.selected_for_deletion)
    );

    let targets = search.deletion_targets().expect("targets");
    assert_eq!(targets.len(), 1);
    assert!(targets[0].exists(), "the search never deletes anything");
}

#[test]
fn reuses_cached_digests_across_runs() {
    let (fixture, _payload) = fixture();
    let result = scan_tree(&fixture);
    let cache = HashCache::open_in_memory().expect("cache");
    let options = DuplicateOptions::default();

    let first = find_duplicates(&result.tree, options, Some(&cache)).expect("first");
    let misses_after_first = cache.misses();
    let second = find_duplicates(&result.tree, options, Some(&cache)).expect("second");

    assert_eq!(first.groups.len(), second.groups.len());
    assert!(cache.hits() >= 2, "the second run must reuse digests");
    assert_eq!(cache.misses(), misses_after_first);
    assert!(cache.len().expect("len") >= 2);
}

#[test]
fn keep_rules_leave_exactly_one_copy() {
    let (fixture, _payload) = fixture();
    let result = scan_tree(&fixture);
    let search = find_duplicates(&result.tree, DuplicateOptions::default(), None).expect("search");
    let mut group = search.groups[0].clone();

    for rule in [
        KeepRule::Oldest,
        KeepRule::Newest,
        KeepRule::Shortest,
        KeepRule::OriginalDir,
    ] {
        apply_keep_rule(&mut group, rule);
        assert_eq!(group.selected_count(), group.members.len() - 1);
        assert!(group.keep_node_id.is_some());
    }
}

#[test]
fn honours_the_minimum_size() {
    let (fixture, payload) = fixture();
    let result = scan_tree(&fixture);
    let search = find_duplicates(
        &result.tree,
        DuplicateOptions {
            min_size: u64::try_from(payload.len()).expect("size") + 1,
            ..DuplicateOptions::default()
        },
        None,
    )
    .expect("search");

    assert!(search.groups.is_empty());
}
