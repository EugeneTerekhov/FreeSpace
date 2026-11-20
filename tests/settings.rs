//! US23: settings, language switching and the portable storage/build policy.

mod common;

use std::{collections::BTreeSet, path::Path};

use freespace_core::{
    i18n::{self, Locale},
    settings::{
        CONFIG_KEYS, DATA_DIR_NAME, SETTINGS_FILE_NAME, Settings, Theme, data_root, default_path,
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
        .set_value("context_menu_enabled", "false")
        .expect("context menu");
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
fn application_data_stays_beside_the_executable() {
    let directory = tempfile::tempdir().expect("temp dir");
    let data = directory.path().join(DATA_DIR_NAME);
    let path = settings_path(&data);

    assert_eq!(path, data.join(SETTINGS_FILE_NAME));

    let mut settings = Settings::default();
    settings.set_value("language", "ru").expect("language");
    settings.save(&path).expect("save settings");

    assert!(path.is_file());
    assert!(
        !directory.path().join("AppData").exists(),
        "no user-profile directory may be created"
    );
}

#[test]
fn default_path_is_application_owned() {
    let path = default_path().expect("default path");
    let executable = std::env::current_exe().expect("current executable");
    let expected = executable
        .parent()
        .expect("executable parent")
        .join(DATA_DIR_NAME)
        .join(SETTINGS_FILE_NAME);

    assert_eq!(path, expected);
    assert_eq!(
        data_root().expect("data root"),
        expected.parent().expect("parent")
    );
    let text = path.to_string_lossy().to_ascii_lowercase();
    for forbidden in ["appdata", "programdata", "\\temp"] {
        assert!(
            !text.contains(forbidden),
            "settings must not use `{forbidden}`: {text}"
        );
    }
}

#[test]
fn context_menu_is_disabled_by_default() {
    assert!(!Settings::default().context_menu_enabled);
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
