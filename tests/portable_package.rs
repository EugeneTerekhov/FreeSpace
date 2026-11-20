//! Regression test for the portable build configuration.
//!
//! The portable archives are produced by plain `cargo build --release`, not by
//! the Tauri CLI. Tauri treats a build as a development build unless the
//! `custom-protocol` feature is enabled, and a development build loads
//! `devUrl` (`http://localhost:1420`) instead of the frontend embedded from
//! `ui/dist`. The shipped archive then opens an empty window with
//! `ERR_CONNECTION_REFUSED`.
//!
//! The packaging scripts must therefore enable the feature explicitly, and the
//! desktop crate must keep forwarding it to `tauri`. This test inspects the
//! manifests and the scripts so the mistake cannot come back unnoticed.

use std::path::{Path, PathBuf};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn read(relative: &str) -> String {
    let path = workspace().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

#[test]
fn desktop_crate_forwards_the_custom_protocol_feature() {
    let manifest = read("apps/freespace-desktop/src-tauri/Cargo.toml");

    assert!(
        manifest.contains("custom-protocol = [\"tauri/custom-protocol\"]"),
        "the desktop crate must forward `custom-protocol` to tauri"
    );
}

#[test]
fn portable_scripts_enable_the_custom_protocol_feature() {
    for script in ["installer/portable.ps1", "installer/portable.sh"] {
        let text = read(script);
        assert!(
            text.contains("freespace-desktop/custom-protocol"),
            "{script} must build with `--features freespace-desktop/custom-protocol`"
        );
        assert!(
            text.contains("--release"),
            "{script} must build the release profile"
        );
    }
}
