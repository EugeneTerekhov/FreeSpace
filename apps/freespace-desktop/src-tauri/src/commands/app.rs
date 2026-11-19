//! Shell commands: bootstrap, capabilities, settings, window state and about.

use tauri::{AppHandle, Emitter, State};

use freespace_app::{
    dto::{
        AboutInfo, AppBootstrap, AppCapabilities, ElevationResponse, ElevationStatus,
        NavigationRequest, UiSettings, UiSettingsPatch, WindowState,
    },
    error::{UiError, UiErrorCode},
    validation,
    window_state::MonitorBounds,
};

use crate::{
    events::{EventEnvelope, SettingsChangedEvent},
    state::AppState,
};

/// Collects monitor work areas for window-state sanitisation.
#[must_use]
pub fn monitor_bounds(app: &AppHandle) -> Vec<MonitorBounds> {
    app.available_monitors()
        .unwrap_or_default()
        .into_iter()
        .map(|monitor| MonitorBounds {
            left: monitor.position().x,
            top: monitor.position().y,
            width: monitor.size().width,
            height: monitor.size().height,
            name: monitor.name().cloned(),
        })
        .collect()
}

/// Returns everything the shell needs before the first screen renders.
#[tauri::command(rename_all = "snake_case")]
pub fn app_bootstrap(app: AppHandle, state: State<'_, AppState>) -> Result<AppBootstrap, UiError> {
    Ok(state.bootstrap(
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        option_env!("WEBVIEW2_VERSION").map(str::to_owned),
        &monitor_bounds(&app),
    ))
}

/// Returns the command and event registry.
#[tauri::command(rename_all = "snake_case")]
#[must_use]
pub fn app_capabilities() -> AppCapabilities {
    crate::security::capabilities()
}

/// Returns the current interface settings.
#[tauri::command(rename_all = "snake_case")]
pub fn app_settings_get(state: State<'_, AppState>) -> Result<UiSettings, UiError> {
    Ok(state.ui_settings())
}

/// Applies and persists a settings patch.
#[tauri::command(rename_all = "snake_case")]
pub fn app_settings_set(
    app: AppHandle,
    state: State<'_, AppState>,
    patch: UiSettingsPatch,
) -> Result<UiSettings, UiError> {
    let settings = state.update_settings(patch)?;
    let envelope = EventEnvelope::new(
        "settings-changed",
        state.next_event_sequence(),
        None,
        SettingsChangedEvent {
            settings: settings.clone(),
        },
    );
    let _ = app.emit("settings-changed", envelope);

    Ok(settings)
}

/// Returns the persisted window geometry, sanitised for the current monitors.
#[tauri::command(rename_all = "snake_case")]
pub fn app_window_state_get(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<WindowState, UiError> {
    Ok(state.window_state().sanitized(&monitor_bounds(&app)))
}

/// Stores and persists window geometry.
#[tauri::command(rename_all = "snake_case")]
pub fn app_window_state_set(
    app: AppHandle,
    state: State<'_, AppState>,
    window_state: WindowState,
) -> Result<(), UiError> {
    state.set_window_state(window_state, &monitor_bounds(&app))
}

/// Returns the About payload.
#[tauri::command(rename_all = "snake_case")]
pub fn app_about_info(app: AppHandle, state: State<'_, AppState>) -> Result<AboutInfo, UiError> {
    Ok(state
        .bootstrap(
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            None,
            &monitor_bounds(&app),
        )
        .about)
}

/// Validates a navigation request coming from a deep link.
#[tauri::command(rename_all = "snake_case")]
pub fn app_navigation_request(request: NavigationRequest) -> Result<(), UiError> {
    if let Some(path) = &request.path {
        validation::validate_path(path)?;
    }

    Ok(())
}

/// Returns the elevation status of the process.
#[tauri::command(rename_all = "snake_case")]
#[must_use]
pub fn app_elevation_status() -> ElevationStatus {
    let elevated = freespace_platform::elevation::is_elevated();

    ElevationStatus {
        elevated,
        can_elevate: !elevated,
        reason_key: (!elevated).then(|| "elevation.reason.mft".to_owned()),
    }
}

/// Asks the shell to relaunch FreeSpace with administrative rights.
///
/// The user sees the native UAC prompt. A declined or impossible request is
/// reported as `E_ELEVATION_DENIED`; no data is modified in either case.
#[tauri::command(rename_all = "snake_case")]
pub fn app_elevation_request(reason_key: String) -> Result<ElevationResponse, UiError> {
    if reason_key.trim().is_empty() {
        return Err(UiError::validation("an elevation reason is required"));
    }
    if freespace_platform::elevation::is_elevated() {
        return Ok(ElevationResponse { elevated: true });
    }

    let executable = freespace_platform::elevation::current_executable()
        .ok_or_else(|| denied("the FreeSpace executable cannot be resolved"))?;
    let arguments = freespace_platform::elevation::relaunch_arguments();

    match freespace_platform::elevation::relaunch(&executable, &arguments) {
        Ok(freespace_platform::elevation::ElevationResult::Launched) => {
            Ok(ElevationResponse { elevated: true })
        }
        Ok(freespace_platform::elevation::ElevationResult::Declined) => {
            Err(denied("the user declined the elevation prompt"))
        }
        Err(error) => Err(denied(&error.to_string())),
    }
}

fn denied(details: &str) -> UiError {
    UiError::new(UiErrorCode::ElevationDenied).with_details(details)
}

/// Settings sections exposed to the client.
pub const SETTINGS_SECTIONS: [&str; 10] = [
    "theme",
    "language",
    "density",
    "units",
    "reduced_motion",
    "last_screen",
    "sidebar",
    "column_layouts",
    "saved_filters",
    "all",
];

#[cfg(test)]
mod tests {
    use super::app_elevation_request;

    #[test]
    fn elevation_needs_a_reason_and_never_relaunches_in_tests() {
        let error = app_elevation_request(String::new()).expect_err("empty reason");
        assert_eq!(error.code, freespace_app::error::UiErrorCode::Validation);

        let error = app_elevation_request("   ".to_owned()).expect_err("blank reason");
        assert_eq!(error.code, freespace_app::error::UiErrorCode::Validation);
    }
}
