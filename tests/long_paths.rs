#![cfg(windows)]

//! US11: rename, move, copy and delete for paths beyond `MAX_PATH`.

mod common;

use common::TestTree;
use freespace_ops::long_path_ops::{copy_to, delete, move_to, rename};
use freespace_platform::paths::MAX_PATH_CHARS;

#[test]
fn operates_on_paths_longer_than_260_characters() {
    let fixture = TestTree::new();
    let directory = fixture.long_directory(270);
    let source = directory.join("payload.bin");
    std::fs::write(&source, [0xAB; 512]).expect("write");
    assert!(source.as_os_str().len() > 260);

    let renamed = rename(&source, "renamed.bin").expect("rename");
    let renamed_path = renamed.to.expect("target");
    assert!(renamed_path.exists());
    assert_eq!(renamed.bytes, 512);

    let destination = fixture.mkdir("destination");
    let moved = move_to(&renamed_path, &destination).expect("move");
    let moved_path = moved.to.expect("target");
    assert!(moved_path.exists());

    let copied = copy_to(&moved_path, fixture.root()).expect("copy");
    let copied_path = copied.to.expect("target");
    assert_eq!(std::fs::read(&copied_path).expect("read"), [0xAB; 512]);

    delete(&copied_path).expect("delete copy");
    delete(&moved_path).expect("delete moved");
    assert!(!copied_path.exists());
    assert!(!moved_path.exists());
}

#[test]
fn operates_on_paths_around_one_thousand_characters() {
    let fixture = TestTree::new();
    let directory = fixture.long_directory(1_000);
    let source = directory.join("deep.bin");
    std::fs::write(&source, b"deep payload").expect("write");
    assert!(source.as_os_str().len() > 1_000);

    let renamed = rename(&source, "deeper.bin").expect("rename");
    let renamed_path = renamed.to.expect("target");
    assert!(renamed_path.exists());

    delete(&renamed_path).expect("delete");
    assert!(!renamed_path.exists());
}

#[test]
fn refuses_a_name_that_would_exceed_the_limit_before_changing_data() {
    let fixture = TestTree::new();
    let path = fixture.write("source.txt", b"payload");

    let error = rename(&path, &"x".repeat(MAX_PATH_CHARS)).expect_err("too long");

    assert_eq!(error.code().as_str(), "E_INVALID_PATH");
    assert_eq!(std::fs::read(&path).expect("read"), b"payload");
}

#[test]
fn refuses_to_copy_into_its_own_subtree() {
    let fixture = TestTree::new();
    let directory = fixture.mkdir("folder");
    fixture.write("folder/nested/file.bin", b"payload");

    let error = copy_to(&directory, &directory.join("nested")).expect_err("own subtree");

    assert_eq!(error.code().as_str(), "E_INVALID_ARGUMENT");
}
