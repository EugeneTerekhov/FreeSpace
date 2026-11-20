//! Persistent interface settings under `[ui]` in `data/settings.toml`.
//!
//! The 001 settings file is still written by `freespace-core`, so this module
//! owns a TOML document around it: unknown root keys are preserved, the legacy
//! `language`, `theme` and `units` keys are mirrored into `[ui]`, and the first
//! migration keeps a `settings.toml.bak` copy. The migration is idempotent.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use freespace_core::settings::{DuplicateKeepRule, Settings, Theme};
use freespace_core::{i18n::Locale, size::Units};

use crate::{
    dto::{
        FilterSet, Language, PathValidation, ScreenId, SettingsPatch, SettingsView,
        ThemePreference, UiSettings, UiSettingsPatch, UnitSystem,
    },
    error::UiError,
    validation::{MAX_EXCLUSION_CHARS, MAX_EXCLUSIONS, UI_SCHEMA_VERSION, validate_ui_settings},
};

/// Name of the settings file inside the data directory.
pub const SETTINGS_FILE_NAME: &str = "settings.toml";
/// Suffix of the one-time pre-migration backup.
pub const BACKUP_SUFFIX: &str = "toml.bak";
/// Largest accepted retention or staleness threshold, in days.
pub const MAX_DAY_THRESHOLD: u32 = 3_650;
/// Largest accepted monitoring interval, in seconds.
pub const MAX_MONITOR_INTERVAL_SECS: u64 = 86_400;
/// Largest number of paths checked by one `settings_validate_paths` call.
pub const MAX_VALIDATED_PATHS: usize = 64;

/// A section that `app_settings_reset` can restore to defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingsSection {
    /// Theme preference.
    Theme,
    /// Interface language.
    Language,
    /// Row density.
    Density,
    /// Unit system.
    Units,
    /// Reduced-motion preference.
    ReducedMotion,
    /// Last open screen.
    LastScreen,
    /// Sidebar state.
    Sidebar,
    /// Column layouts.
    ColumnLayouts,
    /// Saved filters.
    SavedFilters,
    /// Every UI section.
    All,
}

impl SettingsSection {
    /// Stable wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Theme => "theme",
            Self::Language => "language",
            Self::Density => "density",
            Self::Units => "units",
            Self::ReducedMotion => "reduced_motion",
            Self::LastScreen => "last_screen",
            Self::Sidebar => "sidebar",
            Self::ColumnLayouts => "column_layouts",
            Self::SavedFilters => "saved_filters",
            Self::All => "all",
        }
    }

    /// Parses a wire name.
    #[must_use]
    pub fn parse_value(value: &str) -> Option<Self> {
        [
            Self::Theme,
            Self::Language,
            Self::Density,
            Self::Units,
            Self::ReducedMotion,
            Self::LastScreen,
            Self::Sidebar,
            Self::ColumnLayouts,
            Self::SavedFilters,
            Self::All,
        ]
        .into_iter()
        .find(|section| section.as_str() == value)
    }
}

impl From<Theme> for ThemePreference {
    fn from(theme: Theme) -> Self {
        match theme {
            Theme::System => Self::System,
            Theme::Dark => Self::Dark,
            Theme::Light => Self::Light,
        }
    }
}

impl From<ThemePreference> for Theme {
    fn from(theme: ThemePreference) -> Self {
        match theme {
            ThemePreference::System => Self::System,
            ThemePreference::Dark => Self::Dark,
            ThemePreference::Light => Self::Light,
        }
    }
}

impl From<Locale> for Language {
    fn from(locale: Locale) -> Self {
        match locale {
            Locale::Ru => Self::Ru,
            Locale::En => Self::En,
        }
    }
}

impl From<Language> for Locale {
    fn from(language: Language) -> Self {
        match language {
            Language::Ru => Self::Ru,
            Language::En => Self::En,
        }
    }
}

impl From<Units> for UnitSystem {
    fn from(units: Units) -> Self {
        match units {
            Units::Binary => Self::Binary,
            Units::Decimal => Self::Decimal,
        }
    }
}

impl From<UnitSystem> for Units {
    fn from(units: UnitSystem) -> Self {
        match units {
            UnitSystem::Binary => Self::Binary,
            UnitSystem::Decimal => Self::Decimal,
        }
    }
}

/// Loaded settings document with its `[ui]` section.
#[derive(Debug, Clone)]
pub struct SettingsStore {
    path: PathBuf,
    document: toml::Value,
    core: Settings,
    ui: UiSettings,
}

impl SettingsStore {
    /// Loads and, when needed, migrates `settings.toml`.
    ///
    /// # Errors
    /// Returns `E_VALIDATION` when the file is not valid TOML or `[ui]` contains
    /// invalid values, and `E_IO` when the backup cannot be written.
    pub fn load(path: impl Into<PathBuf>) -> Result<Self, UiError> {
        let path = path.into();
        let existed = path.exists();
        let text = if existed {
            fs::read_to_string(&path)?
        } else {
            String::new()
        };

        let document: toml::Value = if text.trim().is_empty() {
            toml::Value::Table(toml::map::Map::new())
        } else {
            toml::from_str(&text)
                .map_err(|error| UiError::validation(format!("invalid settings.toml: {error}")))?
        };
        let core = if text.trim().is_empty() {
            Settings::default()
        } else {
            Settings::from_toml(&text)?
        };

        let (ui, migrated) = if !existed {
            (UiSettings::default(), false)
        } else {
            match document.get("ui") {
                Some(table) => {
                    let mut settings = table.clone().try_into::<UiSettings>().map_err(|error| {
                        UiError::validation(format!("invalid [ui] section: {error}"))
                    })?;
                    // A CLI `config set` edits the legacy root keys without
                    // knowing `[ui]`; an explicit root value therefore wins so
                    // the documented CLI contract stays effective.
                    if document.get("theme").is_some() {
                        settings.theme = core.theme.into();
                    }
                    if document.get("language").is_some() {
                        settings.language = core.language.into();
                    }
                    if document.get("units").is_some() {
                        settings.units = core.units.into();
                    }
                    (settings, false)
                }
                None => (ui_from_core(&core), true),
            }
        };
        validate_ui_settings(&ui)?;

        let store = Self {
            path,
            document,
            core,
            ui,
        };

        if migrated && existed {
            store.backup_once()?;
        }

        Ok(store)
    }

    /// Settings file path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Legacy settings kept for the CLI and for the other commands.
    #[must_use]
    pub fn core(&self) -> &Settings {
        &self.core
    }

    /// Current interface settings.
    #[must_use]
    pub fn ui(&self) -> &UiSettings {
        &self.ui
    }

    /// Builds the payload of `settings_get`.
    #[must_use]
    pub fn view(&self) -> SettingsView {
        SettingsView {
            language: self.core.language.into(),
            theme: self.core.theme.into(),
            units: self.core.units.into(),
            exclusions: self.core.exclusions.clone(),
            duplicate_keep_rule: self.core.duplicate_keep_rule.as_str().to_owned(),
            outdated_threshold_days: self.core.outdated_threshold_days,
            monitor_threshold_bytes: self.core.monitor_threshold.threshold_bytes,
            monitor_interval_secs: self.core.monitor_threshold.interval_secs,
            context_menu_enabled: self.core.context_menu_enabled,
            confirm_destructive_actions: self.core.confirm_destructive_actions,
            ui: self.ui.clone(),
        }
    }

    /// Applies a patch in memory after validation; call [`Self::save`] to persist.
    ///
    /// # Errors
    /// Returns `E_VALIDATION` for an invalid resulting value and leaves the
    /// previous state untouched.
    pub fn apply_patch(&mut self, patch: UiSettingsPatch) -> Result<UiSettings, UiError> {
        let mut next = self.ui.clone();
        apply_ui_patch(&mut next, patch);

        validate_ui_settings(&next)?;
        self.ui = next;
        self.mirror_into_core();

        Ok(self.ui.clone())
    }

    /// Applies a patch and persists the document.
    ///
    /// # Errors
    /// Propagates validation and write failures.
    pub fn update(&mut self, patch: UiSettingsPatch) -> Result<UiSettings, UiError> {
        let settings = self.apply_patch(patch)?;
        self.save()?;

        Ok(settings)
    }

    /// Applies a full settings patch (001 keys plus `[ui]`) and persists it.
    ///
    /// Unknown root keys stored by a newer build are preserved because
    /// [`Self::save`] merges the previous document.
    ///
    /// # Errors
    /// Returns `E_VALIDATION` for an invalid value and `E_IO` when the file
    /// cannot be written. Nothing is persisted when validation fails.
    pub fn update_view(&mut self, patch: SettingsPatch) -> Result<SettingsView, UiError> {
        let mut next_core = self.core.clone();
        let mut next_ui = self.ui.clone();

        if let Some(ui_patch) = patch.ui {
            apply_ui_patch(&mut next_ui, ui_patch);
        }
        if let Some(language) = patch.language {
            next_ui.language = language;
        }
        if let Some(theme) = patch.theme {
            next_ui.theme = theme;
        }
        if let Some(units) = patch.units {
            next_ui.units = units;
        }
        if let Some(exclusions) = patch.exclusions {
            next_core.exclusions = validate_exclusions(&exclusions)?;
        }
        if let Some(rule) = patch.duplicate_keep_rule {
            next_core.duplicate_keep_rule =
                DuplicateKeepRule::parse_value(&rule).ok_or_else(|| {
                    UiError::validation(format!(
                        "unknown duplicate keep rule `{rule}`; expected `oldest`, `newest`, \
                         `shortest-path` or `original-dir`"
                    ))
                })?;
        }
        if let Some(days) = patch.outdated_threshold_days {
            next_core.outdated_threshold_days = validate_days("outdated_threshold_days", days)?;
        }
        if let Some(threshold) = patch.monitor_threshold_bytes {
            next_core.monitor_threshold.threshold_bytes = threshold;
        }
        if let Some(interval) = patch.monitor_interval_secs {
            if !(1..=MAX_MONITOR_INTERVAL_SECS).contains(&interval) {
                return Err(UiError::validation(format!(
                    "monitor_interval_secs must be between 1 and {MAX_MONITOR_INTERVAL_SECS}"
                )));
            }
            next_core.monitor_threshold.interval_secs = interval;
        }
        if let Some(enabled) = patch.context_menu_enabled {
            next_core.context_menu_enabled = enabled;
        }
        if let Some(confirm) = patch.confirm_destructive_actions {
            next_core.confirm_destructive_actions = confirm;
        }

        next_core.theme = next_ui.theme.into();
        next_core.language = next_ui.language.into();
        next_core.units = next_ui.units.into();
        validate_ui_settings(&next_ui)?;

        self.core = next_core;
        self.ui = next_ui;
        self.save()?;

        Ok(self.view())
    }

    /// Restores one section to its default value and persists the document.
    ///
    /// # Errors
    /// Propagates validation and write failures.
    pub fn reset_section(&mut self, section: SettingsSection) -> Result<UiSettings, UiError> {
        let defaults = UiSettings::default();

        match section {
            SettingsSection::Theme => self.ui.theme = defaults.theme,
            SettingsSection::Language => self.ui.language = defaults.language,
            SettingsSection::Density => self.ui.density = defaults.density,
            SettingsSection::Units => self.ui.units = defaults.units,
            SettingsSection::ReducedMotion => self.ui.reduced_motion = defaults.reduced_motion,
            SettingsSection::LastScreen => self.ui.last_screen = defaults.last_screen,
            SettingsSection::Sidebar => self.ui.sidebar_collapsed = defaults.sidebar_collapsed,
            SettingsSection::ColumnLayouts => {
                self.ui.column_layouts = defaults.column_layouts;
            }
            SettingsSection::SavedFilters => self.ui.saved_filters = defaults.saved_filters,
            SettingsSection::All => self.ui = defaults,
        }

        validate_ui_settings(&self.ui)?;
        self.mirror_into_core();
        self.save()?;

        Ok(self.ui.clone())
    }

    /// Writes the document atomically, preserving unknown root keys.
    ///
    /// # Errors
    /// Returns `E_IO` for write failures and `E_INTERNAL` when TOML cannot be
    /// produced.
    pub fn save(&mut self) -> Result<(), UiError> {
        validate_ui_settings(&self.ui)?;

        let mut base = toml::Value::try_from(&self.core)?;
        let base_table = base
            .as_table_mut()
            .ok_or_else(|| UiError::new(crate::error::UiErrorCode::Internal))?;

        // Preserve keys this build does not know, so a newer CLI never loses them.
        if let Some(document) = self.document.as_table() {
            for (key, value) in document {
                if key != "ui" && !base_table.contains_key(key) {
                    base_table.insert(key.clone(), value.clone());
                }
            }
        }

        base_table.insert(
            "language".to_owned(),
            toml::Value::String(self.core.language.code().to_owned()),
        );
        base_table.insert(
            "theme".to_owned(),
            toml::Value::String(self.core.theme.as_str().to_owned()),
        );
        base_table.insert(
            "units".to_owned(),
            toml::Value::String(self.core.units.as_str().to_owned()),
        );
        base_table.insert("ui".to_owned(), toml::Value::try_from(&self.ui)?);

        let text = toml::to_string_pretty(&base)?;
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }

        let temporary = self.path.with_extension("toml.tmp");
        fs::write(&temporary, text)?;
        fs::rename(&temporary, &self.path)?;
        self.document = base;

        Ok(())
    }

    /// One-time copy of the pre-migration file.
    fn backup_once(&self) -> Result<(), UiError> {
        let backup = self.path.with_extension(BACKUP_SUFFIX);
        if backup.exists() {
            return Ok(());
        }

        fs::copy(&self.path, &backup)?;
        Ok(())
    }

    /// Keeps the legacy keys consistent with the `[ui]` section.
    fn mirror_into_core(&mut self) {
        self.core.theme = self.ui.theme.into();
        self.core.language = self.ui.language.into();
        self.core.units = self.ui.units.into();
    }
}

/// Seeds `[ui]` from the legacy root keys during the first migration.
fn ui_from_core(core: &Settings) -> UiSettings {
    UiSettings {
        theme: core.theme.into(),
        language: core.language.into(),
        units: core.units.into(),
        schema_version: UI_SCHEMA_VERSION,
        ..UiSettings::default()
    }
}

fn apply_ui_patch(settings: &mut UiSettings, patch: UiSettingsPatch) {
    if let Some(theme) = patch.theme {
        settings.theme = theme;
    }
    if let Some(language) = patch.language {
        settings.language = language;
    }
    if let Some(density) = patch.density {
        settings.density = density;
    }
    if let Some(units) = patch.units {
        settings.units = units;
    }
    if let Some(reduced_motion) = patch.reduced_motion {
        settings.reduced_motion = reduced_motion;
    }
    if let Some(last_screen) = patch.last_screen {
        settings.last_screen = Some(last_screen);
    }
    if let Some(sidebar_collapsed) = patch.sidebar_collapsed {
        settings.sidebar_collapsed = sidebar_collapsed;
    }
    if let Some(column_layouts) = patch.column_layouts {
        settings.column_layouts = column_layouts;
    }
    if let Some(saved_filters) = patch.saved_filters {
        settings.saved_filters = saved_filters;
    }
}

fn validate_exclusions(exclusions: &[String]) -> Result<Vec<String>, UiError> {
    if exclusions.len() > MAX_EXCLUSIONS {
        return Err(UiError::validation(format!(
            "at most {MAX_EXCLUSIONS} exclusions are supported"
        )));
    }
    let mut validated = Vec::with_capacity(exclusions.len());
    for exclusion in exclusions {
        let trimmed = exclusion.trim();
        if trimmed.is_empty() || trimmed.chars().count() > MAX_EXCLUSION_CHARS {
            return Err(UiError::validation(format!(
                "exclusion must be 1..={MAX_EXCLUSION_CHARS} characters"
            )));
        }
        if trimmed.contains('\0') {
            return Err(UiError::validation("exclusion contains a NUL character"));
        }
        validated.push(trimmed.to_owned());
    }

    Ok(validated)
}

fn validate_days(name: &str, days: u32) -> Result<u32, UiError> {
    if days > MAX_DAY_THRESHOLD {
        return Err(UiError::validation(format!(
            "{name} must be at most {MAX_DAY_THRESHOLD}"
        )));
    }

    Ok(days)
}

/// Checks user-supplied paths for the settings screen.
///
/// Validation follows `crate::validation` first; the file system is only
/// touched for absolute, syntactically valid paths. `writable` is `true` when a
/// directory can be listed or a file is not marked read-only; the function never
/// creates, writes, moves or deletes anything.
///
/// # Errors
/// Returns `E_VALIDATION` when more than [`MAX_VALIDATED_PATHS`] paths are sent.
pub fn validate_paths(paths: &[String]) -> Result<Vec<PathValidation>, UiError> {
    if paths.len() > MAX_VALIDATED_PATHS {
        return Err(UiError::validation(format!(
            "at most {MAX_VALIDATED_PATHS} paths can be checked at once"
        )));
    }

    Ok(paths.iter().map(|path| validate_one_path(path)).collect())
}

fn validate_one_path(text: &str) -> PathValidation {
    let path = match crate::validation::validate_path(text) {
        Ok(path) => path,
        Err(_) => {
            return PathValidation {
                path: text.to_owned(),
                valid: false,
                exists: false,
                is_directory: false,
                parent_exists: parent_exists(text),
                writable: false,
                error_key: Some("settings.path.invalid".to_owned()),
            };
        }
    };

    match fs::canonicalize(&path) {
        Ok(canonical) => match fs::metadata(&canonical) {
            Ok(metadata) => {
                let is_directory = metadata.is_dir();
                let writable = if is_directory {
                    fs::read_dir(&canonical).is_ok()
                } else {
                    !metadata.permissions().readonly()
                };

                PathValidation {
                    path: text.to_owned(),
                    valid: writable || metadata.is_file(),
                    exists: true,
                    is_directory,
                    parent_exists: true,
                    writable,
                    error_key: (!writable && is_directory)
                        .then(|| "settings.path.denied".to_owned()),
                }
            }
            Err(error) => PathValidation {
                path: text.to_owned(),
                valid: false,
                exists: true,
                is_directory: false,
                parent_exists: true,
                writable: false,
                error_key: Some(reason_key_for(&error).to_owned()),
            },
        },
        Err(error) => PathValidation {
            path: text.to_owned(),
            valid: false,
            exists: error.kind() != std::io::ErrorKind::NotFound,
            is_directory: false,
            parent_exists: parent_exists(text),
            writable: false,
            error_key: Some(reason_key_for(&error).to_owned()),
        },
    }
}

fn parent_exists(text: &str) -> bool {
    Path::new(text)
        .parent()
        .is_some_and(|parent| parent.as_os_str().is_empty() || parent.exists())
}

fn reason_key_for(error: &std::io::Error) -> &'static str {
    match error.kind() {
        std::io::ErrorKind::NotFound => "settings.path.missing",
        std::io::ErrorKind::PermissionDenied => "settings.path.denied",
        _ => "settings.path.unavailable",
    }
}

/// Resolves `data/settings.toml` from a data directory.
#[must_use]
pub fn settings_path(data_dir: &Path) -> PathBuf {
    data_dir.join(SETTINGS_FILE_NAME)
}

/// Saves saved filters for one screen, enforcing the per-screen limit.
///
/// # Errors
/// Returns `E_VALIDATION` when more than 20 filters are supplied.
pub fn set_saved_filters(
    settings: &mut UiSettings,
    screen: ScreenId,
    filters: Vec<FilterSet>,
) -> Result<(), UiError> {
    if filters.len() > crate::validation::MAX_SAVED_FILTERS {
        return Err(UiError::validation(format!(
            "at most {} filters can be saved per screen",
            crate::validation::MAX_SAVED_FILTERS
        )));
    }

    settings.saved_filters.insert(screen, filters);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{SETTINGS_FILE_NAME, SettingsSection, SettingsStore};
    use crate::dto::{
        ColumnState, FilterSet, Language, ScreenId, ThemePreference, UiSettingsPatch, UnitSystem,
    };
    use std::fs;

    fn legacy_text() -> &'static str {
        r#"
language = "en"
theme = "dark"
units = "decimal"
exclusions = ["**/node_modules"]

[future_section]
value = 42
"#
    }

    #[test]
    fn migrates_legacy_keys_and_keeps_a_backup() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join(SETTINGS_FILE_NAME);
        fs::write(&path, legacy_text()).expect("write legacy");

        let mut store = SettingsStore::load(&path).expect("load");
        let backup = path.with_extension(super::BACKUP_SUFFIX);

        assert_eq!(store.ui().theme, ThemePreference::Dark);
        assert_eq!(store.ui().language, Language::En);
        assert_eq!(store.ui().units, UnitSystem::Decimal);
        assert_eq!(store.ui().schema_version, 1);
        assert_eq!(fs::read_to_string(&backup).expect("backup"), legacy_text());

        store.save().expect("save");
        let text = fs::read_to_string(&path).expect("read saved");
        assert!(text.contains("[ui]"), "{text}");
        assert!(text.contains("future_section"), "unknown keys must survive");
        assert!(text.contains("value = 42"));

        let reloaded = SettingsStore::load(&path).expect("reload");
        assert_eq!(reloaded.ui(), store.ui());
        assert_eq!(
            fs::read_to_string(&backup).expect("backup"),
            legacy_text(),
            "the backup is never overwritten"
        );
    }

    #[test]
    fn creates_defaults_without_a_backup() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("nested").join(SETTINGS_FILE_NAME);

        let mut store = SettingsStore::load(&path).expect("load");
        assert_eq!(store.ui(), &crate::dto::UiSettings::default());
        assert!(!path.with_extension(super::BACKUP_SUFFIX).exists());

        let settings = store
            .update(UiSettingsPatch {
                theme: Some(ThemePreference::Light),
                ..UiSettingsPatch::default()
            })
            .expect("update");
        assert_eq!(settings.theme, ThemePreference::Light);
        assert!(path.exists());
        assert!(!path.with_extension(super::BACKUP_SUFFIX).exists());
    }

    #[test]
    fn patches_round_trip_and_validate() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join(SETTINGS_FILE_NAME);
        let mut store = SettingsStore::load(&path).expect("load");

        let mut columns = std::collections::BTreeMap::new();
        columns.insert(
            ScreenId::Tree,
            vec![ColumnState {
                id: "name".to_owned(),
                visible: true,
                width_px: 320,
                order: 0,
                sortable: true,
            }],
        );
        store
            .update(UiSettingsPatch {
                language: Some(Language::En),
                column_layouts: Some(columns),
                ..UiSettingsPatch::default()
            })
            .expect("update");

        let reloaded = SettingsStore::load(&path).expect("reload");
        assert_eq!(reloaded.ui().language, Language::En);
        assert_eq!(reloaded.ui().column_layouts.len(), 1);
        assert_eq!(
            reloaded.core().language.code(),
            "en",
            "legacy CLI keys are mirrored"
        );

        let mut invalid = std::collections::BTreeMap::new();
        invalid.insert(
            ScreenId::Tree,
            vec![FilterSet::default(); crate::validation::MAX_SAVED_FILTERS + 1],
        );
        let error = store
            .apply_patch(UiSettingsPatch {
                saved_filters: Some(invalid),
                ..UiSettingsPatch::default()
            })
            .expect_err("must reject");
        assert_eq!(error.code, crate::error::UiErrorCode::Validation);
        assert!(store.ui().saved_filters.is_empty(), "state is unchanged");
    }

    #[test]
    fn resets_sections() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join(SETTINGS_FILE_NAME);
        let mut store = SettingsStore::load(&path).expect("load");
        store
            .update(UiSettingsPatch {
                theme: Some(ThemePreference::Dark),
                language: Some(Language::En),
                ..UiSettingsPatch::default()
            })
            .expect("update");

        let reset = store
            .reset_section(SettingsSection::Theme)
            .expect("reset theme");
        assert_eq!(reset.theme, ThemePreference::System);
        assert_eq!(reset.language, Language::En);

        let reset = store
            .reset_section(SettingsSection::All)
            .expect("reset all");
        assert_eq!(reset, crate::dto::UiSettings::default());
    }

    #[test]
    fn exposes_and_updates_the_full_settings_view() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join(SETTINGS_FILE_NAME);
        let mut store = SettingsStore::load(&path).expect("load");

        let view = store
            .update_view(crate::dto::SettingsPatch {
                ui: Some(UiSettingsPatch {
                    theme: Some(ThemePreference::Light),
                    density: Some(crate::dto::Density::Compact),
                    ..UiSettingsPatch::default()
                }),
                theme: Some(ThemePreference::Dark),
                exclusions: Some(vec!["**/node_modules".to_owned()]),
                duplicate_keep_rule: Some("newest".to_owned()),
                outdated_threshold_days: Some(90),
                monitor_threshold_bytes: Some(4_096),
                monitor_interval_secs: Some(5),
                confirm_destructive_actions: Some(false),
                ..crate::dto::SettingsPatch::default()
            })
            .expect("update view");

        assert_eq!(view.theme, ThemePreference::Dark);
        assert_eq!(view.ui.theme, ThemePreference::Dark);
        assert_eq!(view.ui.density, crate::dto::Density::Compact);
        assert_eq!(view.exclusions, ["**/node_modules"]);
        assert_eq!(view.duplicate_keep_rule, "newest");
        assert_eq!(view.monitor_interval_secs, 5);

        let reloaded = SettingsStore::load(&path).expect("reload");
        assert_eq!(reloaded.view(), view);
        assert_eq!(reloaded.core().duplicate_keep_rule.as_str(), "newest");

        let legacy = freespace_core::settings::Settings::load(&path).expect("cli load");
        assert_eq!(legacy.get_value("theme").as_deref(), Some("dark"));
        assert_eq!(
            legacy.get_value("exclusions").as_deref(),
            Some("**/node_modules")
        );
        assert_eq!(
            legacy.get_value("monitor_interval_secs").as_deref(),
            Some("5")
        );

        let error = store
            .update_view(crate::dto::SettingsPatch {
                monitor_interval_secs: Some(0),
                ..crate::dto::SettingsPatch::default()
            })
            .expect_err("interval");
        assert_eq!(error.code, crate::error::UiErrorCode::Validation);
        assert_eq!(store.view(), view, "rejected patches change nothing");

        let error = store
            .update_view(crate::dto::SettingsPatch {
                duplicate_keep_rule: Some("magic".to_owned()),
                ..crate::dto::SettingsPatch::default()
            })
            .expect_err("rule");
        assert_eq!(error.code, crate::error::UiErrorCode::Validation);
    }

    #[test]
    fn legacy_root_keys_win_over_a_stale_ui_section() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join(SETTINGS_FILE_NAME);
        fs::write(
            &path,
            r#"
theme = "dark"
language = "en"
units = "decimal"

[ui]
theme = "light"
language = "ru"
units = "binary"
density = "compact"
"#,
        )
        .expect("write");

        let store = SettingsStore::load(&path).expect("load");
        assert_eq!(store.ui().theme, ThemePreference::Dark);
        assert_eq!(store.ui().language, Language::En);
        assert_eq!(store.ui().units, UnitSystem::Decimal);
        assert_eq!(store.ui().density, crate::dto::Density::Compact);
        assert_eq!(store.view().theme, ThemePreference::Dark);
    }

    #[test]
    fn validates_existing_missing_and_malformed_paths() {
        let directory = tempfile::tempdir().expect("temp dir");
        let file = directory.path().join("payload.bin");
        fs::write(&file, b"x").expect("write");
        let missing = directory.path().join("missing");
        let paths = vec![
            directory.path().to_string_lossy().into_owned(),
            file.to_string_lossy().into_owned(),
            missing.to_string_lossy().into_owned(),
            "relative\\folder".to_owned(),
        ];

        let results = super::validate_paths(&paths).expect("validate");
        assert!(results[0].valid);
        assert!(results[0].exists);
        assert!(results[0].is_directory);
        assert!(results[1].valid);
        assert!(!results[2].valid);
        assert!(!results[2].exists);
        assert_eq!(
            results[2].error_key.as_deref(),
            Some("settings.path.missing")
        );
        assert!(!results[3].valid);
        assert_eq!(
            results[3].error_key.as_deref(),
            Some("settings.path.invalid")
        );

        let many = vec!["C:\\".to_owned(); super::MAX_VALIDATED_PATHS + 1];
        let error = super::validate_paths(&many).expect_err("too many");
        assert_eq!(error.code, crate::error::UiErrorCode::Validation);
    }
}
