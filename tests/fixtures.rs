//! Integration tests for the shared filesystem fixtures themselves.
//!
//! These tests validate that the fixture utilities are deterministic and safe
//! before other test suites rely on them.

mod common;

use common::{TestTree, admin_tests_enabled, require_admin, sample_sizes};
use freespace_core::size::SizeMetrics;

#[test]
fn sample_fixture_has_deterministic_contents() {
    let fixture = TestTree::sample();
    let expected: u64 = sample_sizes().iter().sum();

    assert_eq!(fixture.total_file_bytes(), expected);

    let total: SizeMetrics = sample_sizes()
        .iter()
        .map(|size| SizeMetrics::file(*size))
        .sum();
    assert_eq!(total.logical, expected);
}

#[test]
fn fixture_supports_unicode_names() {
    let fixture = TestTree::sample();
    let path = fixture.path("UNICODE-имя/файл.txt");

    assert!(path.is_file());
    assert_eq!(
        std::fs::read_to_string(path).expect("read unicode fixture"),
        "hello"
    );
}

#[test]
fn fixture_creates_paths_beyond_the_legacy_limit() {
    let fixture = TestTree::new();
    let directory = fixture.long_directory(300);

    assert!(directory.as_os_str().len() > 260);
    assert!(directory.is_dir());

    let file = directory.join("payload.bin");
    std::fs::write(
        freespace_platform::paths::to_verbatim(&file).expect("verbatim file path"),
        b"payload",
    )
    .expect("write long fixture file");

    assert_eq!(
        std::fs::read(file).expect("read long fixture file"),
        b"payload"
    );
}

#[test]
fn administrative_tests_are_opt_in() {
    let enabled = admin_tests_enabled();

    assert_eq!(
        enabled,
        std::env::var("FREESPACE_ADMIN_TESTS").as_deref() == Ok("1")
    );
    assert_eq!(require_admin().is_some(), enabled);
}
