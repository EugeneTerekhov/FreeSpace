//! US4: top files and the type report on a real scanned tree.

mod common;

use common::TestTree;
use freespace_analysis::{FileCategory, group_by_category, group_by_extension, top_files};
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

fn fixture() -> TestTree {
    let fixture = TestTree::new();
    fixture.write("a.bin", &[b'a'; 100]);
    fixture.write("b.bin", &[b'b'; 300]);
    fixture.write("c.txt", &[b'c'; 200]);
    fixture.write("nested/d.bin", &[b'd'; 500]);
    fixture.mkdir("empty");

    fixture
}

#[test]
fn top_files_are_ordered_and_limited() {
    let fixture = fixture();
    let result = scan_tree(&fixture);

    let all = top_files(&result.tree, 100);
    let sizes: Vec<u64> = all.iter().map(|file| file.size.logical).collect();
    assert_eq!(sizes, vec![500, 300, 200, 100]);
    assert_eq!(all.iter().map(|file| file.size.logical).sum::<u64>(), 1_100);

    let two = top_files(&result.tree, 2);
    assert_eq!(two.len(), 2);
    assert_eq!(two[0].size.logical, 500);
    assert_eq!(two[1].size.logical, 300);
    assert!(top_files(&result.tree, 0).is_empty());
    assert!(all.iter().all(|file| file.category != FileCategory::Other));
}

#[test]
fn type_report_sums_extensions_and_categories() {
    let fixture = fixture();
    let result = scan_tree(&fixture);

    let groups = group_by_extension(&result.tree);
    let bin = groups
        .iter()
        .find(|group| group.extension == "bin")
        .expect("bin group");
    assert_eq!(bin.count, 3);
    assert_eq!(bin.size.logical, 900);
    assert_eq!(bin.category, FileCategory::Executable);
    assert!((bin.share - 900.0 / 1_100.0).abs() < 1e-9);

    let txt = groups
        .iter()
        .find(|group| group.extension == "txt")
        .expect("txt group");
    assert_eq!(txt.count, 1);
    assert_eq!(txt.size.logical, 200);
    assert_eq!(txt.category, FileCategory::Document);

    let categories = group_by_category(&result.tree);
    let total: u64 = categories
        .iter()
        .map(|category| category.size.logical)
        .sum();
    assert_eq!(total, 1_100);
    assert_eq!(
        categories
            .iter()
            .map(|category| category.count)
            .sum::<u64>(),
        4
    );
    assert_eq!(groups.len(), 2, "no empty extensions on this fixture");
}
