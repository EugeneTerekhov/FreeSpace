//! Contract test: the advertised [`Capabilities`] match the actual backend.

use crate::{Capabilities, platform};

#[test]
fn capabilities_match_the_selected_backend() {
    let caps = platform().capabilities;

    assert_eq!(caps, Capabilities::current());
    assert!(caps.permanent, "permanent deletion is mandatory");

    if cfg!(windows) {
        assert_eq!(caps.platform.as_str(), "windows");
        assert!(caps.scan_methods.mft);
        assert!(caps.scan_methods.usn);
        assert!(caps.trash);
        assert!(caps.shred);
        assert!(caps.acl);
        assert!(caps.attrs);
        assert!(caps.ads);
        assert!(caps.unlock);
        assert!(caps.monitor);
        assert!(caps.elevation);
        assert!(caps.long_paths);
        assert!(!caps.case_sensitive);
        assert_eq!(caps.max_path, None);
    } else if cfg!(target_os = "linux") {
        assert_eq!(caps.platform.as_str(), "linux");
        assert!(!caps.scan_methods.mft);
        assert!(!caps.scan_methods.usn);
        assert!(caps.scan_methods.walk);
        assert!(caps.trash);
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
        assert_eq!(caps.max_path, Some(4096));
    }
}

#[test]
fn path_facade_delegates_to_the_backend() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("fixture.txt");
    std::fs::write(&path, b"payload").expect("write fixture");

    let normalized = crate::paths::normalize(&path).expect("normalize");
    assert_eq!(
        normalized,
        platform()
            .paths
            .normalize(&path)
            .expect("platform normalize")
    );
    platform()
        .paths
        .validate_length(&normalized)
        .expect("length within the platform limit");

    let root = platform().paths.volume_root(&normalized).expect("root");
    let root = platform().paths.normalize(&root).expect("normalize root");
    assert!(normalized.starts_with(&root));
    platform()
        .paths
        .ensure_within(&root, &normalized)
        .expect("fixture is inside its volume root");
}

#[test]
fn metadata_capabilities_match_actual_calls() {
    let caps = platform().capabilities;
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("fixture.bin");
    std::fs::write(&path, vec![0x5A; 4096]).expect("write fixture");

    let allocated = platform()
        .metadata
        .allocated_size(&path)
        .expect("allocated size");
    assert!(allocated > 0);
    assert!(platform().metadata.owner(&path).expect("owner").is_some());

    if caps.attrs {
        let mask = platform().metadata.attributes(&path).expect("attributes");
        platform()
            .metadata
            .set_attributes(&path, 0, mask)
            .expect("clear attributes");
    } else {
        assert_eq!(
            platform()
                .metadata
                .attributes(&path)
                .expect_err("attributes must be unsupported")
                .code()
                .as_str(),
            "E_UNSUPPORTED"
        );
    }

    if caps.ads {
        let streams = platform().metadata.streams(&path).expect("streams");
        assert!(streams.is_empty(), "fixtures have no named streams");
    } else {
        assert!(
            platform()
                .metadata
                .streams(&path)
                .expect("streams are empty on this platform")
                .is_empty()
        );
    }
}

#[test]
fn shred_capability_matches_the_backend() {
    let caps = platform().capabilities;
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("disposable.bin");
    std::fs::write(&path, b"payload").expect("write fixture");

    let preview = platform().shred.preview(&path);

    if caps.shred {
        let preview = preview.expect("preview");
        assert!(preview.size > 0);

        platform()
            .shred
            .run(&path, crate::common::shred::ShredOptions::default())
            .expect("shred");
        assert!(!path.exists());
    } else {
        assert_eq!(
            preview.expect_err("unsupported").code().as_str(),
            "E_UNSUPPORTED"
        );
        assert!(path.exists(), "an unsupported call must not delete files");
    }
}

#[cfg(unix)]
#[test]
fn windows_only_services_are_unsupported_on_unix() {
    let caps = platform().capabilities;
    assert!(!caps.shred);
    assert!(!caps.unlock);
    assert!(!caps.context_menu);

    assert_eq!(
        platform()
            .unlock
            .find_locks(std::path::Path::new("/"))
            .expect_err("unlock must be unsupported")
            .code()
            .as_str(),
        "E_UNSUPPORTED"
    );
    assert_eq!(
        platform()
            .storage
            .trim(std::path::Path::new("/"))
            .expect_err("trim must be unsupported")
            .code()
            .as_str(),
        "E_UNSUPPORTED"
    );
}
