//! Theme, language and reduced-motion injection before the first paint.
//!
//! Rust reads `settings.toml` before the window exists and passes the resolved
//! boot state through `initialization_script`, so the shell never flashes the
//! wrong theme. The WebView2 profile is forced into `data/webview2`.

use std::path::{Path, PathBuf};

use freespace_app::dto::{Language, ReducedMotion, ThemePreference, UiSettings};
use serde::Serialize;

/// Theme, language and motion handed to the synchronous boot script.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BootState {
    /// Theme preference; `system` is resolved by `boot.js`.
    pub theme: ThemePreference,
    /// Interface language.
    pub language: Language,
    /// Reduced-motion preference.
    pub reduced_motion: ReducedMotion,
}

impl From<&UiSettings> for BootState {
    fn from(settings: &UiSettings) -> Self {
        Self {
            theme: settings.theme,
            language: settings.language,
            reduced_motion: settings.reduced_motion,
        }
    }
}

/// Additional WebView2 arguments that disable background networking.
///
/// Wry already disables `msWebOOUI`, `msPdfOOUI` and `msSmartScreenProtection`;
/// overriding the argument list requires repeating them.
pub const WEBVIEW2_BROWSER_ARGS: &str = "--disable-background-networking \
--disable-component-update --disable-sync --no-first-run \
--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,msEdgeSidebarV2";

/// Returns the script that publishes `window.__FREESPACE_BOOT__`.
#[must_use]
pub fn initialization_script(state: &BootState) -> String {
    let json = serde_json::to_string(state).unwrap_or_else(|_| "{}".to_owned());

    format!("window.__FREESPACE_BOOT__ = Object.freeze({json});")
}

/// Returns the WebView2 profile directory under `data`.
#[must_use]
pub fn webview2_data_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("webview2")
}

/// Minimum WebView2 runtime the application supports.
pub const MINIMUM_WEBVIEW2_VERSION: &str = "109.0.1518.78";

/// Returns the fixed WebView2 runtime shipped next to the executable, if any.
///
/// The portable build may carry a `webview2` folder with an extracted fixed
/// runtime. WebView2 loads it when `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER` points
/// at that folder before the first webview is created.
#[must_use]
pub fn fixed_runtime_dir(exe_dir: &Path) -> Option<PathBuf> {
    let runtime = exe_dir.join("webview2");
    runtime
        .join("msedgewebview2.exe")
        .is_file()
        .then_some(runtime)
}

/// Selects the bundled fixed runtime for the process.
///
/// Must run before the Tauri runtime starts. The portable archive relies on it;
/// an installed copy falls back to the machine-wide Evergreen runtime.
pub fn apply_fixed_runtime(exe_dir: &Path) {
    if let Some(runtime) = fixed_runtime_dir(exe_dir) {
        // SAFETY: this runs at the very start of `run`, before Tauri or WebView2
        // spawn any thread that could read the environment concurrently.
        unsafe {
            std::env::set_var("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER", runtime);
        }
    }
}

/// Returns true when `version` is at least [`MINIMUM_WEBVIEW2_VERSION`].
///
/// Versions are compared as dot-separated numbers; a malformed value is treated
/// as unsupported so the caller can warn instead of crashing.
#[must_use]
pub fn webview2_supported(version: &str) -> bool {
    fn parts(value: &str) -> Option<Vec<u64>> {
        value
            .split('.')
            .map(|part| part.parse::<u64>().ok())
            .collect()
    }

    let Some(found) = parts(version) else {
        return false;
    };
    let Some(required) = parts(MINIMUM_WEBVIEW2_VERSION) else {
        return true;
    };

    for (index, expected) in required.iter().enumerate() {
        let actual = found.get(index).copied().unwrap_or(0);
        match actual.cmp(expected) {
            std::cmp::Ordering::Greater => return true,
            std::cmp::Ordering::Less => return false,
            std::cmp::Ordering::Equal => {}
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use freespace_app::dto::{Language, ReducedMotion, ThemePreference, UiSettings};

    use super::{
        BootState, MINIMUM_WEBVIEW2_VERSION, WEBVIEW2_BROWSER_ARGS, fixed_runtime_dir,
        initialization_script, webview2_data_dir, webview2_supported,
    };

    #[test]
    fn publishes_the_documented_boot_object() {
        let settings = UiSettings {
            theme: ThemePreference::Dark,
            language: Language::En,
            reduced_motion: ReducedMotion::On,
            ..UiSettings::default()
        };
        let script = initialization_script(&BootState::from(&settings));

        assert!(script.starts_with("window.__FREESPACE_BOOT__ = Object.freeze("));
        assert!(script.contains("\"theme\":\"dark\""));
        assert!(script.contains("\"language\":\"en\""));
        assert!(script.contains("\"reduced_motion\":\"on\""));
        assert!(script.ends_with(");"));
    }

    #[test]
    fn resolves_the_profile_inside_data() {
        let data = std::path::Path::new(r"C:\Tools\FreeSpace\data");
        assert_eq!(
            webview2_data_dir(data),
            std::path::PathBuf::from(r"C:\Tools\FreeSpace\data\webview2")
        );
    }

    #[test]
    fn disables_background_networking() {
        assert!(WEBVIEW2_BROWSER_ARGS.contains("--disable-background-networking"));
        assert!(WEBVIEW2_BROWSER_ARGS.contains("--disable-component-update"));
        assert!(WEBVIEW2_BROWSER_ARGS.contains("msSmartScreenProtection"));
        assert!(!WEBVIEW2_BROWSER_ARGS.contains("remote-debugging"));
    }

    #[test]
    fn finds_a_bundled_fixed_runtime() {
        let root = tempfile::tempdir().expect("temp dir");
        let runtime = root.path().join("webview2");
        std::fs::create_dir_all(&runtime).expect("webview2 dir");
        std::fs::write(runtime.join("msedgewebview2.exe"), b"").expect("runtime binary");

        assert_eq!(fixed_runtime_dir(root.path()), Some(runtime));
        assert_eq!(fixed_runtime_dir(&root.path().join("missing")), None);
    }

    #[test]
    fn compares_the_webview2_version() {
        assert!(webview2_supported(MINIMUM_WEBVIEW2_VERSION));
        assert!(webview2_supported("130.0.2849.68"));
        assert!(webview2_supported("110.0.0.0"));
        assert!(webview2_supported("109.0.1518.79"));
        assert!(!webview2_supported("108.0.0.0"));
        assert!(!webview2_supported("109.0.1518.0"));
        assert!(!webview2_supported("not-a-version"));
        assert!(!webview2_supported(""));
    }
}
