//! US23: settings, language switching and portable mode.

mod common;

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use freespace_core::{
    i18n::{self, Locale},
    settings::{
        CONFIG_KEYS, PORTABLE_MARKER, SETTINGS_FILE_NAME, Settings, Theme, default_path,
        settings_path,
    },
    size::Units,
};

#[test]
fn switches_language_without_losing_state() {
    let mut settings = Settings::default();
    settings
        .set_value("exclusions", r"C:\Windows\Temp")
        .expect("exclusions");

    settings.set_value("language", "ru").expect("language");
    settings.apply_language();
    assert_eq!(i18n::locale(), Locale::Ru);
    assert_eq!(settings.exclusions, [r"C:\Windows\Temp"]);

    settings.set_value("language", "en").expect("language");
    settings.apply_language();
    assert_eq!(i18n::locale(), Locale::En);
    assert_eq!(settings.exclusions, [r"C:\Windows\Temp"]);
}

#[test]
fn rejects_unknown_languages_and_keeps_the_previous_one() {
    let mut settings = Settings::default();
    settings.set_value("language", "ru").expect("language");

    assert!(settings.set_value("language", "de").is_err());
    assert_eq!(settings.language, Locale::Ru);
}

#[test]
fn round_trips_every_supported_key() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join(SETTINGS_FILE_NAME);
    let mut settings = Settings::default();

    settings.set_value("theme", "dark").expect("theme");
    settings.set_value("units", "decimal").expect("units");
    settings
        .set_value("duplicate_keep_rule", "newest")
        .expect("rule");
    settings
        .set_value("outdated_threshold_days", "42")
        .expect("days");
    settings
        .set_value("quarantine_retention_days", "7")
        .expect("retention");
    settings
        .set_value("portable_mode", "true")
        .expect("portable");
    settings
        .set_value("confirm_destructive_actions", "false")
        .expect("confirm");
    settings
        .set_value("monitor_threshold_bytes", "2048")
        .expect("threshold");
    settings
        .set_value("monitor_interval_secs", "3")
        .expect("interval");
    settings
        .set_value("exclusions", r"C:\Temp;*.tmp")
        .expect("exclusions");
    settings.save(&path).expect("save");

    let loaded = Settings::load(&path).expect("load");
    assert_eq!(loaded, settings);
    for key in CONFIG_KEYS {
        assert!(loaded.get_value(key).is_some(), "missing {key}");
    }
    assert_eq!(loaded.theme, Theme::Dark);
    assert_eq!(loaded.units, Units::Decimal);
    assert_eq!(loaded.exclusions, [r"C:\Temp", "*.tmp"]);
}

#[test]
fn portable_mode_keeps_files_next_to_the_executable() {
    let directory = tempfile::tempdir().expect("temp dir");
    let exe_dir = directory.path().join("FreeSpace");
    std::fs::create_dir_all(&exe_dir).expect("exe dir");
    std::fs::write(exe_dir.join(PORTABLE_MARKER), b"").expect("marker");
    let profile = directory.path().join("AppData");
    std::fs::create_dir_all(&profile).expect("profile");

    let portable_path = settings_path(&exe_dir, Some(&profile), true);
    assert_eq!(portable_path, exe_dir.join(SETTINGS_FILE_NAME));

    let mut settings = Settings::default();
    settings.set_value("language", "ru").expect("language");
    settings
        .save(&portable_path)
        .expect("save portable settings");

    assert!(portable_path.is_file());
    let profile_settings = profile.join("FreeSpace").join(SETTINGS_FILE_NAME);
    assert!(
        !profile_settings.exists(),
        "portable mode must not write to the user profile"
    );
}

#[test]
fn roaming_mode_uses_appdata() {
    let exe = Path::new(r"C:\Tools\FreeSpace");
    let app_data = Path::new(r"C:\Users\user\AppData\Roaming");
    let path: PathBuf = settings_path(exe, Some(app_data), false);

    assert_eq!(
        path,
        Path::new(r"C:\Users\user\AppData\Roaming\FreeSpace\settings.toml")
    );
}

#[test]
fn default_path_matches_an_existing_marker_or_profile() {
    // The current process either has a portable marker beside the binary or an
    // APPDATA variable; both cases must yield a path ending with settings.toml.
    if let Ok(path) = default_path() {
        assert!(path.ends_with(SETTINGS_FILE_NAME));
    }
}

#[test]
fn translation_files_share_the_same_key_set() {
    let locales = Path::new(env!("CARGO_MANIFEST_DIR")).join("../locales");
    let english = keys(&locales.join("en.toml"));
    let russian = keys(&locales.join("ru.toml"));

    assert_eq!(english, russian, "locales must contain the same keys");
}

fn keys(path: &Path) -> BTreeSet<String> {
    let text = std::fs::read_to_string(path).expect("read locale");
    let value: toml::Value = toml::from_str(&text).expect("parse locale");
    let mut keys = BTreeSet::new();
    flatten("", &value, &mut keys);

    keys
}

fn flatten(prefix: &str, value: &toml::Value, keys: &mut BTreeSet<String>) {
    if let Some(table) = value.as_table() {
        for (key, value) in table {
            let full = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            flatten(&full, value, keys);
        }
    } else {
        keys.insert(prefix.to_owned());
    }
}
