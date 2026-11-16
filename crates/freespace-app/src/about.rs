//! About payload and the offline third-party licence list (US6, T129).
//!
//! The screen shows the version, the build label, the WebView2 version when the
//! shell knows it, the `data` directory and a static licence list. Nothing here
//! performs a network request: URLs are never opened automatically and only
//! textual identifiers are exposed.

use std::path::Path;

use crate::dto::{AboutInfo, LicenseInfo};

/// Environment variable that carries the build label at compile time.
pub const BUILD_ENV: &str = "FREESPACE_BUILD";

/// Build label used when the build script did not provide one.
pub const DEFAULT_BUILD_LABEL: &str = "dev";

/// Returns the compile-time build label.
#[must_use]
pub fn build_label() -> String {
    option_env!("FREESPACE_BUILD")
        .unwrap_or(DEFAULT_BUILD_LABEL)
        .to_owned()
}

/// Static licence list shipped inside the binary.
///
/// The list is intentionally offline: `url` stays `None` so the interface can
/// show the project name without ever opening a browser.
#[must_use]
pub fn licenses() -> Vec<LicenseInfo> {
    [
        ("Tauri", "MIT / Apache-2.0"),
        ("React", "MIT"),
        ("Lucide icons", "ISC"),
        (
            "Microsoft Edge WebView2",
            "Microsoft Software License Terms",
        ),
        ("Rust standard library", "MIT / Apache-2.0"),
        ("FreeSpace Rust crates", "MIT / Apache-2.0"),
    ]
    .into_iter()
    .map(|(name, license)| LicenseInfo {
        name: name.to_owned(),
        license: license.to_owned(),
        url: None,
    })
    .collect()
}

/// Builds the `about_info` payload.
#[must_use]
pub fn about_info(data_dir: &Path, version: &str, webview_version: Option<String>) -> AboutInfo {
    AboutInfo {
        version: version.to_owned(),
        build: build_label(),
        webview_version,
        licenses: licenses(),
        data_dir: data_dir.to_string_lossy().into_owned(),
    }
}

/// Returns only the licence list for `about_licenses`.
#[must_use]
pub fn licenses_payload() -> Vec<LicenseInfo> {
    licenses()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{about_info, build_label, licenses, licenses_payload};

    #[test]
    fn describes_the_build_without_network_links() {
        let info = about_info(
            Path::new(r"C:\Tools\FreeSpace\data"),
            "1.1.0",
            Some("120.0.0".to_owned()),
        );

        assert_eq!(info.version, "1.1.0");
        assert_eq!(info.build, build_label());
        assert!(!info.build.is_empty());
        assert_eq!(info.webview_version.as_deref(), Some("120.0.0"));
        assert_eq!(info.data_dir, r"C:\Tools\FreeSpace\data");
        assert!(!info.licenses.is_empty());
        for license in &info.licenses {
            assert!(!license.name.is_empty());
            assert!(!license.license.is_empty());
            assert!(
                license.url.is_none(),
                "the About screen never opens links: {license:?}"
            );
        }
    }

    #[test]
    fn the_licence_list_is_stable_and_offline() {
        let first = licenses();
        let second = licenses_payload();

        assert_eq!(first, second);
        assert!(first.iter().any(|license| license.name == "Tauri"));
        assert!(first.iter().any(|license| license.name == "React"));
        for license in first {
            let text = format!("{} {}", license.name, license.license);
            assert!(!text.contains("http"), "no network references: {text}");
        }
    }

    #[test]
    fn about_info_accepts_an_unknown_webview_version() {
        let info = about_info(Path::new(r"C:\data"), "1.0.0", None);
        assert!(info.webview_version.is_none());
    }
}
