//! Settings and About commands (US6, T128–T129).
//!
//! The commands are thin adapters: validation, persistence and migration live in
//! `freespace_app::settings`, while the About payload is built by
//! `freespace_app::about`. Both `settings_set` and `settings_reset` emit the same
//! `settings-changed` envelope as `app_settings_set`, so the shell has a single
//! synchronisation path.

use tauri::{AppHandle, Emitter, State};

use freespace_app::{
    about,
    dto::{AboutInfo, LicenseInfo, PathValidation, SettingsPatch, SettingsView},
    error::UiError,
    settings::{self, SettingsSection},
};

use crate::{
    events::{EventEnvelope, SettingsChangedEvent},
    state::AppState,
};

/// Returns the 001 settings plus the migrated `[ui]` section.
#[tauri::command(rename_all = "snake_case")]
pub fn settings_get(state: State<'_, AppState>) -> Result<SettingsView, UiError> {
    Ok(state.settings_view())
}

/// Applies a partial settings patch and persists it.
#[tauri::command(rename_all = "snake_case")]
pub fn settings_set(
    app: AppHandle,
    state: State<'_, AppState>,
    patch: SettingsPatch,
) -> Result<SettingsView, UiError> {
    let view = state.update_settings_view(patch)?;
    emit_changed(&app, &state);

    Ok(view)
}

/// Checks paths used by the settings screen without modifying anything.
#[tauri::command(rename_all = "snake_case")]
pub fn settings_validate_paths(paths: Vec<String>) -> Result<Vec<PathValidation>, UiError> {
    settings::validate_paths(&paths)
}

/// Restores one documented section to its default value.
#[tauri::command(rename_all = "snake_case")]
pub fn settings_reset(
    app: AppHandle,
    state: State<'_, AppState>,
    section: String,
) -> Result<SettingsView, UiError> {
    let section = SettingsSection::parse_value(&section).ok_or_else(|| {
        UiError::validation(format!(
            "unknown settings section `{section}`; expected `theme`, `language`, `density`, \
             `units`, `reduced_motion`, `last_screen`, `sidebar`, `column_layouts`, \
             `saved_filters` or `all`"
        ))
    })?;
    let view = state.reset_settings_view(section)?;
    emit_changed(&app, &state);

    Ok(view)
}

/// Returns the About payload built entirely offline.
#[tauri::command(rename_all = "snake_case")]
pub fn about_info(state: State<'_, AppState>) -> Result<AboutInfo, UiError> {
    Ok(about::about_info(
        &state.data_dir(),
        env!("CARGO_PKG_VERSION"),
        option_env!("WEBVIEW2_VERSION").map(str::to_owned),
    ))
}

/// Returns the static third-party licence list.
#[tauri::command(rename_all = "snake_case")]
pub fn about_licenses() -> Result<Vec<LicenseInfo>, UiError> {
    Ok(about::licenses_payload())
}

fn emit_changed(app: &AppHandle, state: &AppState) {
    let envelope = EventEnvelope::new(
        "settings-changed",
        state.next_event_sequence(),
        None,
        SettingsChangedEvent {
            settings: state.ui_settings(),
        },
    );
    let _ = app.emit("settings-changed", envelope);
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    #[test]
    fn about_payloads_are_offline_and_complete() {
        let info = super::about::about_info(Path::new(r"C:\data"), "1.1.0", None);
        assert_eq!(info.version, "1.1.0");
        assert!(!info.build.is_empty());
        assert!(!info.licenses.is_empty());
        assert!(info.data_dir.ends_with("data"));

        let licenses = super::about::licenses_payload();
        assert!(licenses.iter().all(|license| license.url.is_none()));
        assert!(licenses.iter().any(|license| license.name == "Tauri"));
    }
}
