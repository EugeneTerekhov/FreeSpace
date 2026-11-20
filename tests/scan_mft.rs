#![cfg(windows)]

//! US1 MFT tests.
//!
//! These scenarios need administrator rights and a real NTFS volume, so they
//! only run when `FREESPACE_ADMIN_TESTS=1` is set explicitly. The volume is
//! read-only for the test; nothing is written or deleted.

mod common;

use common::require_admin;
use freespace_core::model::{FileSystem, ScanMethod};
use freespace_platform::{mft, paths, volumes};
use freespace_scan::{MethodChoice, NoopObserver, ScanRequest, session::scan};

#[test]
fn enumerates_the_system_volume() {
    let Some(_guard) = require_admin() else {
        return;
    };

    let volume = volumes::describe_volume(std::path::Path::new("C:\\")).expect("volume");
    assert_eq!(volume.file_system, FileSystem::Ntfs, "MFT needs NTFS");
    let record_size = mft::mft_record_size(std::path::Path::new("C:\\")).expect("record size");
    assert!(record_size >= 512, "record size {record_size}");

    let journal = mft::query_journal(std::path::Path::new("C:\\")).expect("journal");
    assert!(journal.journal_id > 0);
    assert!(journal.next_usn > 0);
}

#[test]
fn scans_a_volume_through_the_mft() {
    let Some(_guard) = require_admin() else {
        return;
    };

    let volume = volumes::describe_volume(std::path::Path::new("C:\\")).expect("volume");
    if volume.file_system != FileSystem::Ntfs {
        return;
    }

    let request = ScanRequest {
        method: MethodChoice::Mft,
        ..ScanRequest::new("C:\\")
    };
    let result = scan(&request, &mut NoopObserver).expect("mft scan");
    let root = result.tree.roots().next().expect("root");

    assert_eq!(result.method, ScanMethod::Mft);
    assert!(
        result.session.nodes_count > 10,
        "the system volume has files"
    );
    assert!(result.tree.node(root).size.logical > 0);
    assert!(result.tree.node(root).file_count() > 0);
    assert!(result.tree.node(root).dir_count() > 0);
    assert!(
        result
            .warnings
            .iter()
            .all(|warning| !warning.contains("MFT access"))
    );
    result.tree.validate().expect("consistent tree");

    // The scan must be deterministic when the file system does not change.
    let second = scan(&request, &mut NoopObserver).expect("second scan");
    assert_eq!(second.session.nodes_count, result.session.nodes_count);
}

#[test]
fn increment_reads_the_journal_tail() {
    let Some(_guard) = require_admin() else {
        return;
    };

    let root = paths::to_verbatim(std::path::Path::new("C:\\")).expect("verbatim");
    let state = freespace_scan::usn::capture_state(&root).expect("journal state");
    let read = mft::read_journal(&root, state).expect("journal tail");

    assert_eq!(read.state.journal_id, state.journal_id);
    assert!(read.state.next_usn >= state.next_usn);
    assert!(!read.requires_rescan);
}
