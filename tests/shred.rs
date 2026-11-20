#![cfg(windows)]

//! US6: shredder and deletion-on-reboot guarantees.
//!
//! Only temporary fixture files are destroyed. The test proves that nothing is
//! touched without confirmation, that the file disappears after the overwrite
//! and that the run is journalled with its method and pass count.

mod common;

use common::TestTree;
use freespace_ops::{
    ShredOptions, Store,
    operation::ShredPattern,
    shred::{self, record, shred as destroy},
};
use freespace_platform::trim::{StorageKind, storage_kind};

#[test]
fn refuses_without_confirmation() {
    let fixture = TestTree::new();
    let path = fixture.write("protected.bin", b"payload");

    let error = destroy(&path, ShredOptions::default(), false).expect_err("must refuse");

    assert_eq!(error.code().as_str(), "E_CONFIRMATION_REQUIRED");
    assert_eq!(std::fs::read(&path).expect("read"), b"payload");
}

#[test]
fn destroys_the_file_with_the_requested_passes() {
    let fixture = TestTree::new();
    let path = fixture.write("secret.bin", &[0x5Au8; 8_192]);

    let outcome = destroy(
        &path,
        ShredOptions {
            passes: 2,
            pattern: ShredPattern::Random,
        },
        true,
    )
    .expect("shred");

    assert!(!path.exists(), "the file must not be recoverable by path");
    assert_eq!(outcome.size, 8_192);
    assert_eq!(outcome.passes, 2);
    assert_eq!(outcome.pattern, ShredPattern::Random);
    assert!(matches!(
        outcome.storage,
        StorageKind::Ssd | StorageKind::Hdd | StorageKind::Unknown
    ));
    if storage_kind(fixture.root()).is_ok_and(StorageKind::needs_warning) {
        assert!(
            outcome
                .warnings
                .iter()
                .any(|warning| warning.contains("solid-state")),
            "SSD runs must warn: {:?}",
            outcome.warnings
        );
    }
}

#[test]
fn journals_the_destroyed_files() {
    let fixture = TestTree::new();
    let first = fixture.write("one.bin", &[1u8; 512]);
    let second = fixture.write("two.bin", &[2u8; 1_024]);
    let options = ShredOptions {
        passes: 1,
        pattern: ShredPattern::Zeros,
    };

    let outcomes = vec![
        destroy(&first, options, true).expect("first"),
        destroy(&second, options, true).expect("second"),
    ];
    let store = Store::open_in_memory().expect("store");
    let id = record(&store, &outcomes, "tester").expect("journal");

    let entries = store.journal_list(10).expect("entries");
    assert!(id > 0);
    assert_eq!(entries.len(), 1);
    assert!(entries[0].summary.starts_with("shred"));
    assert_eq!(entries[0].details["passes"], 1);
    assert_eq!(entries[0].details["pattern"], "zeros");
    assert_eq!(
        entries[0].details["targets"]
            .as_array()
            .expect("targets")
            .len(),
        2
    );
    assert!(!first.exists());
    assert!(!second.exists());
}

#[test]
fn rejects_zero_passes_and_directories() {
    let fixture = TestTree::new();
    let path = fixture.write("file.bin", b"payload");

    let error = destroy(
        &path,
        ShredOptions {
            passes: 0,
            pattern: ShredPattern::Zeros,
        },
        true,
    )
    .expect_err("zero passes");
    assert_eq!(error.code().as_str(), "E_INVALID_ARGUMENT");
    assert!(path.exists());

    let error = destroy(fixture.root(), ShredOptions::default(), true).expect_err("directory");
    assert_eq!(error.code().as_str(), "E_INVALID_ARGUMENT");
    assert!(fixture.root().exists());
}

#[test]
fn parses_patterns() {
    assert_eq!(
        ShredPattern::parse_value("zeros"),
        Some(ShredPattern::Zeros)
    );
    assert_eq!(
        ShredPattern::parse_value("random"),
        Some(ShredPattern::Random)
    );
    assert_eq!(ShredPattern::parse_value("magnetic"), None);
    assert_eq!(ShredPattern::Zeros.as_str(), "zeros");
    assert_eq!(ShredOptions::default().passes, 1);
    assert!(!shred::SSD_WARNING.is_empty());
}
