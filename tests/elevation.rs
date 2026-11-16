//! US21: elevation state and access to protected system directories.
//!
//! Administrative assertions run only when `FREESPACE_ADMIN_TESTS=1`; the rest
//! verifies the non-elevated contract.

mod common;

use std::ffi::OsString;
use std::path::Path;

use freespace_platform::elevation;
use freespace_scan::access::{AccessPolicy, Probe, is_protected, protected_areas};

#[test]
fn reports_elevation_and_builds_relaunch_arguments() {
    let elevated = elevation::is_elevated();
    assert_eq!(elevated, elevation::is_administrator());

    let line = elevation::command_line(
        Path::new(r"C:\Program Files\FreeSpace\freespace-gui.exe"),
        &[OsString::from("--json")],
    )
    .expect("command line");
    assert!(line.starts_with('"'));
    assert!(line.contains("--json"));
    assert!(elevation::command_line(Path::new("relative.exe"), &[]).is_err());
}

#[test]
fn lists_protected_areas_for_every_volume() {
    let volumes = freespace_platform::volumes::list_volumes().expect("volumes");
    let volume = volumes
        .iter()
        .find(|volume| volume.status == freespace_core::model::VolumeStatus::Available)
        .expect("available volume");

    let areas = protected_areas(&volume.mount_point);

    assert!(
        areas
            .iter()
            .any(|area| area.name == "System Volume Information")
    );
    assert!(areas.iter().any(|area| area.name == "$Recycle.Bin"));
    assert!(is_protected(Path::new(
        r"C:\System Volume Information\IndexerVolumeGuid"
    )));
}

#[test]
fn limited_policy_explains_denied_system_directories() {
    let policy = AccessPolicy::new(false);
    let path = Path::new(r"C:\System Volume Information");

    assert!(policy.is_limited());
    assert!(policy.requires_elevation(path));
    assert!(
        policy
            .advice(path, &Probe::Denied)
            .contains("administrator")
    );
}

#[test]
fn administrator_can_read_system_volume_information() {
    if !common::admin_tests_enabled() {
        return;
    }

    assert!(
        elevation::is_elevated(),
        "admin tests need an elevated runner"
    );
    let volume = freespace_platform::volumes::list_volumes()
        .expect("volumes")
        .into_iter()
        .find(|volume| volume.status == freespace_core::model::VolumeStatus::Available)
        .expect("volume");
    let path = volume.mount_point.join("System Volume Information");

    assert_eq!(
        freespace_scan::access::probe(&path),
        Probe::Available,
        "an elevated process must be able to enumerate the protected directory"
    );
}
