//! Linux contract tests: advertised capabilities, paths and XDG trash.
//!
//! These run only on the Linux target. They complement the cross-platform
//! [`crate::contract_tests`] by checking the Unix-specific promises: the
//! case-sensitive path key, `PATH_MAX`, the `.trashinfo` layout and the exact
//! error code of every Windows-only service.

use std::path::Path;

use freespace_core::ErrorCode;

use super::{paths, trash};
use crate::{Capabilities, PlatformId, platform};

#[test]
fn capabilities_match_the_linux_backend() {
    let caps = platform().capabilities;

    assert_eq!(caps, Capabilities::linux());
    assert_eq!(caps.platform, PlatformId::Linux);
    assert!(caps.trash);
    assert!(caps.permanent);
    assert!(!caps.shred);
    assert!(!caps.acl);
    assert!(!caps.attrs);
    assert!(!caps.ads);
    assert!(!caps.unlock);
    assert!(!caps.context_menu);
    assert!(caps.monitor);
    assert!(caps.elevation);
    assert!(!caps.long_paths);
    assert!(caps.case_sensitive);
    assert_eq!(caps.max_path, Some(paths::PATH_MAX));
}

#[test]
fn windows_only_services_report_unsupported() {
    let directory = tempfile::tempdir().expect("temp dir");
    let fixture = directory.path().join("fixture.bin");
    std::fs::write(&fixture, b"payload").expect("write fixture");

    let calls = [
        platform()
            .shred
            .preview(&fixture)
            .expect_err("shred is Windows-only")
            .code(),
        platform()
            .unlock
            .find_locks(&fixture)
            .expect_err("unlock is Windows-only")
            .code(),
        platform()
            .context_menu
            .set_enabled(true)
            .expect_err("context menu is Windows-only")
            .code(),
        platform()
            .metadata
            .attributes(&fixture)
            .expect_err("Windows attributes")
            .code(),
        platform()
            .metadata
            .set_attributes(&fixture, 0, 0)
            .expect_err("Windows attributes")
            .code(),
        platform()
            .storage
            .trim(&fixture)
            .expect_err("trim is Windows-only")
            .code(),
    ];

    for code in calls {
        assert_eq!(code, ErrorCode::Unsupported);
    }
    assert!(
        fixture.exists(),
        "unsupported calls must not delete anything"
    );
    assert!(
        platform()
            .metadata
            .streams(&fixture)
            .expect("streams")
            .is_empty()
    );
}

#[test]
fn paths_are_case_sensitive_and_bounded() {
    assert_ne!(
        paths::equivalent_key(Path::new("/tmp/Report.txt")),
        paths::equivalent_key(Path::new("/tmp/report.txt"))
    );

    let too_long = std::path::PathBuf::from("/").join("a".repeat(paths::PATH_MAX));
    assert_eq!(
        paths::validate_length(&too_long)
            .expect_err("one byte over the limit")
            .code(),
        ErrorCode::InvalidPath
    );
    assert!(
        platform()
            .paths
            .ensure_within(Path::new("/tmp/scope"), Path::new("/tmp/scope-other"))
            .is_err()
    );
}

#[test]
fn volume_root_is_a_mount_point() {
    let directory = tempfile::tempdir().expect("temp dir");
    let root = platform()
        .paths
        .volume_root(directory.path())
        .expect("volume root");

    assert!(root.is_absolute());
    assert!(directory.path().starts_with(&root));
}

#[test]
fn xdg_trash_moves_the_object_and_writes_the_record() {
    let directory = tempfile::tempdir().expect("temp dir");
    let trash = trash::TrashPaths::new(&directory.path().join("Trash"));
    let source = directory.path().join("report final.txt");
    std::fs::write(&source, b"payload").expect("write fixture");

    let destination =
        trash::move_to_trash(&source, &trash, "2026-11-20T12:00:00").expect("move to trash");

    assert!(!source.exists());
    assert_eq!(destination, trash.files.join("report final.txt"));
    let record = std::fs::read_to_string(trash.info.join("report final.txt.trashinfo"))
        .expect("read record");
    assert!(record.starts_with("[Trash Info]\n"));
    assert!(record.contains("Path="));
    assert!(record.contains("%20"), "spaces are percent-encoded");
    assert!(record.contains("DeletionDate=2026-11-20T12:00:00"));
}
