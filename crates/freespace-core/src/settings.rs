//! Persistent application settings.
//!
//! FreeSpace never writes to `%APPDATA%`, `%LOCALAPPDATA%`, `%ProgramData%` or
//! temporary directories. Settings, the journal, the quarantine and the hash
//! cache all live in a `data` directory beside the executable. When that folder
//! is read-only the application keeps settings in memory for the session and
//! reports the failure instead of falling back to a system location.
//!
//! The format is versionless on purpose: every field has a default, so old
//! configuration files keep loading after upgrades.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    Error, Result,
    i18n::{self, Locale},
    size::Units,
};

/// File name of the configuration file.
pub const SETTINGS_FILE_NAME: &str = "settings.toml";

/// Name of the application-owned storage directory beside the executable.
pub const DATA_DIR_NAME: &str = "data";

/// Keys exposed by `freespace config get/set`.
pub const CONFIG_KEYS: [&str; 11] = [
    "language",
    "theme",
    "units",
    "duplicate_keep_rule",
    "outdated_threshold_days",
    "quarantine_retention_days",
    "confirm_destructive_actions",
    "monitor_threshold_bytes",
    "monitor_interval_secs",
    "exclusions",
    "context_menu_enabled",
];

/// Colour theme.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Light theme.
    Light,
    /// Dark theme.
    Dark,
    /// Follow the operating-system preference.
    #[default]
    System,
}

impl Theme {
    /// Stable configuration value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::System => "system",
        }
    }

    /// Parses a configuration value.
    #[must_use]
    pub fn parse_value(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            "system" => Some(Self::System),
            _ => None,
        }
    }
}

/// Rule used to pick the copy kept when deleting duplicates.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DuplicateKeepRule {
    /// Keep the oldest file.
    #[default]
    Oldest,
    /// Keep the newest file.
    Newest,
    /// Keep the file with the shortest path.
    ShortestPath,
    /// Keep the copy that already lives in the original directory.
    OriginalDir,
}

impl DuplicateKeepRule {
    /// Stable configuration value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Oldest => "oldest",
            Self::Newest => "newest",
            Self::ShortestPath => "shortest-path",
            Self::OriginalDir => "original-dir",
        }
    }

    /// Parses a configuration value.
    #[must_use]
    pub fn parse_value(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "oldest" => Some(Self::Oldest),
            "newest" => Some(Self::Newest),
            "shortest" | "shortest-path" => Some(Self::ShortestPath),
            "original-dir" => Some(Self::OriginalDir),
            _ => None,
        }
    }
}

/// Threshold that triggers a write-monitoring notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorThreshold {
    /// Number of bytes written within the interval that raises a notification.
    pub threshold_bytes: u64,
    /// Polling interval in seconds.
    pub interval_secs: u64,
}

impl Default for MonitorThreshold {
    fn default() -> Self {
        Self {
            threshold_bytes: 1024 * 1024 * 1024,
            interval_secs: 60,
        }
    }
}

/// User-configurable application settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Interface language.
    pub language: Locale,
    /// Colour theme.
    pub theme: Theme,
    /// Unit system for byte counts.
    pub units: Units,
    /// Path masks excluded from analysis.
    pub exclusions: Vec<String>,
    /// Rule used when deleting duplicates.
    pub duplicate_keep_rule: DuplicateKeepRule,
    /// Age in days after which an object is considered stale.
    pub outdated_threshold_days: u32,
    /// Write-monitoring notification threshold.
    pub monitor_threshold: MonitorThreshold,
    /// How long quarantined objects are retained, in days.
    pub quarantine_retention_days: u32,
    /// Register Explorer context-menu entries (disabled by default).
    pub context_menu_enabled: bool,
    /// Always ask for confirmation before destructive actions.
    pub confirm_destructive_actions: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: Locale::DEFAULT,
            theme: Theme::System,
            units: Units::Binary,
            exclusions: Vec::new(),
            duplicate_keep_rule: DuplicateKeepRule::Oldest,
            outdated_threshold_days: 180,
            monitor_threshold: MonitorThreshold::default(),
            quarantine_retention_days: 30,
            context_menu_enabled: false,
            confirm_destructive_actions: true,
        }
    }
}

impl Settings {
    /// Parses settings from a TOML document; missing fields keep their defaults.
    ///
    /// # Errors
    /// Returns an error when the document is not valid TOML or contains a value
    /// of the wrong type.
    pub fn from_toml(text: &str) -> Result<Self> {
        toml::from_str(text).map_err(|error| Error::Config(error.to_string()))
    }

    /// Serialises settings to TOML.
    ///
    /// # Errors
    /// Returns an error when the settings cannot be represented as TOML.
    pub fn to_toml_string(&self) -> Result<String> {
        toml::to_string_pretty(self).map_err(|error| Error::Config(error.to_string()))
    }

    /// Loads settings from `path`; a missing file yields the defaults.
    ///
    /// # Errors
    /// Returns an error when the file exists but cannot be read or parsed.
    pub fn load(path: &Path) -> Result<Self> {
        match fs::read_to_string(path) {
            Ok(text) => Self::from_toml(&text),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error.into()),
        }
    }

    /// Loads settings from an optional path, using defaults when it is `None`.
    ///
    /// # Errors
    /// Returns an error when the file exists but cannot be read or parsed.
    pub fn load_or_default(path: Option<&Path>) -> Result<Self> {
        match path {
            Some(path) => Self::load(path),
            None => Ok(Self::default()),
        }
    }

    /// Saves settings atomically enough for a single-user desktop app:
    /// parent directories are created, then the file is replaced.
    ///
    /// # Errors
    /// Returns an error when the directory cannot be created or the file cannot
    /// be written.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }

        let text = self.to_toml_string()?;
        let temporary = path.with_extension("toml.tmp");
        fs::write(&temporary, text)?;
        fs::rename(&temporary, path)?;

        Ok(())
    }

    /// Applies the configured language to the runtime translation backend.
    pub fn apply_language(&self) {
        i18n::set_locale(self.language);
    }

    /// Returns the textual value of a configuration key.
    #[must_use]
    pub fn get_value(&self, key: &str) -> Option<String> {
        match key {
            "language" => Some(self.language.code().to_owned()),
            "theme" => Some(self.theme.as_str().to_owned()),
            "units" => Some(self.units.as_str().to_owned()),
            "duplicate_keep_rule" => Some(self.duplicate_keep_rule.as_str().to_owned()),
            "outdated_threshold_days" => Some(self.outdated_threshold_days.to_string()),
            "quarantine_retention_days" => Some(self.quarantine_retention_days.to_string()),
            "context_menu_enabled" => Some(self.context_menu_enabled.to_string()),
            "confirm_destructive_actions" => Some(self.confirm_destructive_actions.to_string()),
            "monitor_threshold_bytes" => Some(self.monitor_threshold.threshold_bytes.to_string()),
            "monitor_interval_secs" => Some(self.monitor_threshold.interval_secs.to_string()),
            "exclusions" => Some(self.exclusions.join(";")),
            _ => None,
        }
    }

    /// Sets a configuration key from its textual value.
    ///
    /// # Errors
    /// Returns [`Error::InvalidArgument`] for unknown keys or malformed values.
    pub fn set_value(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "language" => {
                self.language = Locale::from_code(value).ok_or_else(|| {
                    Error::InvalidArgument(format!(
                        "unknown language `{value}`; expected `ru` or `en`"
                    ))
                })?;
            }
            "theme" => {
                self.theme = Theme::parse_value(value).ok_or_else(|| {
                    Error::InvalidArgument(format!(
                        "unknown theme `{value}`; expected `light`, `dark` or `system`"
                    ))
                })?;
            }
            "units" => {
                self.units = Units::parse_value(value).ok_or_else(|| {
                    Error::InvalidArgument(format!(
                        "unknown units `{value}`; expected `binary` or `decimal`"
                    ))
                })?;
            }
            "duplicate_keep_rule" => {
                self.duplicate_keep_rule =
                    DuplicateKeepRule::parse_value(value).ok_or_else(|| {
                        Error::InvalidArgument(format!(
                            "unknown duplicate keep rule `{value}`; expected `oldest`, `newest`, \
                         `shortest-path` or `original-dir`"
                        ))
                    })?;
            }
            "outdated_threshold_days" => {
                self.outdated_threshold_days = parse_u32(key, value)?;
            }
            "quarantine_retention_days" => {
                self.quarantine_retention_days = parse_u32(key, value)?;
            }
            "context_menu_enabled" => {
                self.context_menu_enabled = parse_bool(key, value)?;
            }
            "confirm_destructive_actions" => {
                self.confirm_destructive_actions = parse_bool(key, value)?;
            }
            "monitor_threshold_bytes" => {
                self.monitor_threshold.threshold_bytes = parse_u64(key, value)?;
            }
            "monitor_interval_secs" => {
                self.monitor_threshold.interval_secs = parse_u64(key, value)?;
            }
            "exclusions" => {
                self.exclusions = if value.trim().is_empty() {
                    Vec::new()
                } else {
                    value
                        .split([';', '|'])
                        .map(str::trim)
                        .filter(|entry| !entry.is_empty())
                        .map(str::to_owned)
                        .collect()
                };
            }
            _ => {
                return Err(Error::InvalidArgument(format!(
                    "unknown configuration key `{key}`; expected one of {}",
                    CONFIG_KEYS.join(", ")
                )));
            }
        }

        Ok(())
    }
}

fn parse_bool(key: &str, value: &str) -> Result<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(Error::InvalidArgument(format!(
            "`{key}` expects a boolean, got `{value}`"
        ))),
    }
}

fn parse_u32(key: &str, value: &str) -> Result<u32> {
    value.parse().map_err(|_| {
        Error::InvalidArgument(format!("`{key}` expects a 32-bit number, got `{value}`"))
    })
}

fn parse_u64(key: &str, value: &str) -> Result<u64> {
    value.parse().map_err(|_| {
        Error::InvalidArgument(format!("`{key}` expects a 64-bit number, got `{value}`"))
    })
}

/// Resolves the directory that contains the running executable.
///
/// # Errors
/// Returns [`Error::Config`] when the executable path cannot be resolved or has
/// no parent directory.
pub fn executable_directory() -> Result<PathBuf> {
    let executable = std::env::current_exe().map_err(|error| {
        Error::Config(format!(
            "cannot resolve the FreeSpace executable path: {error}"
        ))
    })?;
    executable
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| Error::Config("the executable has no parent folder".to_owned()))
}

/// Resolves the application-owned storage directory (`<exe_dir>\data`).
///
/// FreeSpace uses this directory for every persistent artefact: settings, the
/// SQLite journal, quarantine objects, the hash cache and the CLI lock. The
/// function never consults `%APPDATA%`, `%LOCALAPPDATA%`, `%ProgramData%` or a
/// temporary directory.
///
/// # Errors
/// Returns [`Error::Config`] when the executable location is unknown.
pub fn data_root() -> Result<PathBuf> {
    Ok(executable_directory()?.join(DATA_DIR_NAME))
}

/// Joins the settings file to an application data directory.
#[must_use]
pub fn settings_path(data_root: &Path) -> PathBuf {
    data_root.join(SETTINGS_FILE_NAME)
}

/// Resolves the default settings file (`<exe_dir>\data\settings.toml`).
///
/// # Errors
/// Returns [`Error::Config`] when the executable location is unknown.
pub fn default_path() -> Result<PathBuf> {
    Ok(settings_path(&data_root()?))
}

#[cfg(test)]
mod tests {
    use super::{CONFIG_KEYS, DuplicateKeepRule, Settings, Theme, settings_path};
    use crate::{i18n::Locale, size::Units};
    use std::path::Path;

    #[test]
    fn provides_safe_defaults() {
        let settings = Settings::default();

        assert_eq!(settings.language, Locale::En);
        assert_eq!(settings.theme, Theme::System);
        assert_eq!(settings.units, Units::Binary);
        assert_eq!(settings.duplicate_keep_rule, DuplicateKeepRule::Oldest);
        assert!(settings.confirm_destructive_actions);
        assert!(!settings.context_menu_enabled);
        assert_eq!(settings.outdated_threshold_days, 180);
        assert_eq!(settings.quarantine_retention_days, 30);
    }

    #[test]
    fn round_trips_through_toml() {
        let mut settings = Settings::default();
        settings.set_value("language", "ru").expect("language");
        settings.set_value("theme", "dark").expect("theme");
        settings
            .set_value("exclusions", "**/node_modules; *.tmp")
            .expect("exclusions");
        settings
            .set_value("outdated_threshold_days", "90")
            .expect("days");

        let text = settings.to_toml_string().expect("serialise");
        let parsed = Settings::from_toml(&text).expect("parse");

        assert_eq!(parsed, settings);
        assert_eq!(parsed.exclusions, ["**/node_modules", "*.tmp"]);
    }

    #[test]
    fn fills_missing_fields_with_defaults() {
        let settings = Settings::from_toml(
            r#"
            language = "ru"
            theme = "light"
            "#,
        )
        .expect("parse partial settings");

        assert_eq!(settings.language, Locale::Ru);
        assert_eq!(settings.theme, Theme::Light);
        assert_eq!(settings.units, Units::Binary);
        assert!(settings.confirm_destructive_actions);
    }

    #[test]
    fn rejects_invalid_toml() {
        assert!(Settings::from_toml("language = [").is_err());
        assert!(Settings::from_toml("units = \"bits\"").is_err());
    }

    #[test]
    fn reads_and_writes_every_config_key() {
        let mut settings = Settings::default();

        for key in CONFIG_KEYS {
            assert!(
                settings.get_value(key).is_some(),
                "missing getter for {key}"
            );
        }

        settings.set_value("units", "decimal").expect("units");
        settings
            .set_value("duplicate_keep_rule", "shortest")
            .expect("rule");
        settings
            .set_value("context_menu_enabled", "yes")
            .expect("context menu");
        settings
            .set_value("confirm_destructive_actions", "off")
            .expect("confirm");
        settings
            .set_value("monitor_threshold_bytes", "4096")
            .expect("threshold");
        settings
            .set_value("monitor_interval_secs", "15")
            .expect("interval");
        settings
            .set_value("quarantine_retention_days", "7")
            .expect("retention");

        assert_eq!(settings.units, Units::Decimal);
        assert_eq!(
            settings.duplicate_keep_rule,
            DuplicateKeepRule::ShortestPath
        );
        assert!(settings.context_menu_enabled);
        assert!(!settings.confirm_destructive_actions);
        assert_eq!(settings.monitor_threshold.threshold_bytes, 4_096);
        assert_eq!(settings.monitor_threshold.interval_secs, 15);
        assert_eq!(settings.quarantine_retention_days, 7);
        assert_eq!(settings.get_value("units").as_deref(), Some("decimal"));
    }

    #[test]
    fn rejects_unknown_keys_and_bad_values() {
        let mut settings = Settings::default();

        assert!(settings.set_value("nope", "1").is_err());
        assert!(settings.set_value("language", "de").is_err());
        assert!(settings.set_value("theme", "neon").is_err());
        assert!(settings.set_value("units", "bits").is_err());
        assert!(settings.set_value("outdated_threshold_days", "-1").is_err());
        assert!(settings.set_value("context_menu_enabled", "maybe").is_err());
    }

    #[test]
    fn saves_and_loads_a_file() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("nested").join("settings.toml");
        let mut settings = Settings::default();
        settings.set_value("language", "ru").expect("language");

        settings.save(&path).expect("save");
        let loaded = Settings::load(&path).expect("load");

        assert_eq!(loaded, settings);
        assert_eq!(
            Settings::load(&directory.path().join("missing.toml")).expect("default"),
            Settings::default()
        );
    }

    #[test]
    fn keeps_settings_beside_the_executable() {
        let data_root = Path::new(r"C:\Tools\FreeSpace\data");

        assert_eq!(
            settings_path(data_root),
            Path::new(r"C:\Tools\FreeSpace\data\settings.toml")
        );
    }

    #[test]
    fn data_root_is_application_owned() {
        let root = super::data_root().expect("data root");
        let executable = std::env::current_exe().expect("current executable");

        assert_eq!(
            root,
            executable
                .parent()
                .expect("executable parent")
                .join(super::DATA_DIR_NAME)
        );
        let text = root.to_string_lossy().to_ascii_lowercase();
        for forbidden in ["appdata", "programdata", "\\temp"] {
            assert!(
                !text.contains(forbidden),
                "data root must not use `{forbidden}`: {text}"
            );
        }
    }
}
