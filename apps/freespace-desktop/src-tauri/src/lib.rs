//! Tauri application shell for the FreeSpace desktop client.
//!
//! The window is created in Rust so the boot script (theme, language, reduced
//! motion), the WebView2 profile inside `data/webview2` and the offline browser
//! arguments are applied before the first paint.

pub mod boot;
pub mod commands;
pub mod events;
pub mod security;
pub mod state;

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

use crate::{boot::initialization_script, state::AppState};

/// Starts the Tauri runtime.
///
/// # Panics
///
/// Panics when the data directory cannot be resolved, settings cannot be loaded
/// or the window cannot be created; all three are unrecoverable startup failures.
pub fn run() {
    init_tracing();
    select_webview2_runtime();

    let data_dir = state::default_data_dir().expect("cannot resolve the FreeSpace data directory");
    let state = AppState::new(&data_dir).expect("cannot load FreeSpace settings");
    let startup = state.startup_state();
    let boot_script = initialization_script(&startup.boot);

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::app::app_bootstrap,
            commands::app::app_capabilities,
            commands::app::app_settings_get,
            commands::app::app_settings_set,
            commands::app::app_window_state_get,
            commands::app::app_window_state_set,
            commands::app::app_about_info,
            commands::app::app_navigation_request,
            commands::app::app_elevation_status,
            commands::analysis::top_files_page,
            commands::analysis::types_summary,
            commands::analysis::types_files_page,
            commands::analysis::duplicates_start,
            commands::analysis::duplicates_status,
            commands::analysis::duplicates_groups_page,
            commands::analysis::duplicates_group,
            commands::analysis::duplicates_cancel,
            commands::analysis::cleanup_candidates,
            commands::analysis::cleanup_empty_dirs_page,
            commands::analysis::snapshots_list,
            commands::analysis::snapshots_create,
            commands::analysis::snapshots_compare,
            commands::analysis::snapshots_diff_page,
            commands::analysis::snapshots_delete,
            commands::analysis::snapshots_status,
            commands::operations::operation_preview,
            commands::operations::operation_confirm,
            commands::operations::operation_status,
            commands::operations::operation_cancel,
            commands::operations::operation_undo,
            commands::operations::journal_page,
            commands::operations::journal_entry,
            commands::operations::journal_undo,
            commands::filesystem::size_details,
            commands::filesystem::long_paths_page,
            commands::filesystem::long_paths_validate,
            commands::filesystem::rename_preview,
            commands::filesystem::rename_apply,
            commands::filesystem::reveal_in_explorer,
            commands::inspection::metadata_get,
            commands::inspection::owners_page,
            commands::inspection::owners_files_page,
            commands::inspection::archives_entries_page,
            commands::inspection::archives_export_list,
            commands::inspection::export_preview,
            commands::inspection::export_start,
            commands::inspection::export_status,
            commands::inspection::export_cancel,
            commands::inspection::unlock_locks,
            commands::inspection::unlock_release,
            commands::inspection::unlock_schedule_reboot,
            commands::app::app_elevation_request,
            commands::monitor::monitor_start,
            commands::monitor::monitor_stop,
            commands::monitor::monitor_status,
            commands::monitor::monitor_events_page,
            commands::monitor::monitor_set_exclusions,
            commands::monitor::monitor_reveal_executable,
            commands::monitor::monitor_reveal_file,
            commands::settings::settings_get,
            commands::settings::settings_set,
            commands::settings::settings_validate_paths,
            commands::settings::settings_reset,
            commands::settings::about_info,
            commands::settings::about_licenses,
            commands::volumes::volumes_list,
            commands::volumes::volumes_refresh,
            commands::volumes::volumes_summary,
            commands::volumes::volumes_current,
            commands::volumes::snapshot_volumes,
            commands::scan::scan_start,
            commands::scan::scan_status,
            commands::scan::scan_cancel,
            commands::scan::scan_sessions,
            commands::tree::tree_root,
            commands::tree::tree_children,
            commands::tree::tree_search,
            commands::tree::tree_node,
            commands::tree::tree_path,
        ])
        .setup(move |app| {
            let state = app.state::<AppState>();
            let monitors = commands::app::monitor_bounds(app.handle());
            let window_state = state.window_state().sanitized(&monitors);
            let data_dir = state.data_dir();

            if let Ok(version) = tauri::webview_version() {
                if !boot::webview2_supported(&version) {
                    tracing::warn!(
                        %version,
                        minimum = boot::MINIMUM_WEBVIEW2_VERSION,
                        "the WebView2 runtime is older than the supported minimum"
                    );
                }
            }

            let mut builder =
                WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                    .title("FreeSpace")
                    .inner_size(
                        f64::from(window_state.width),
                        f64::from(window_state.height),
                    )
                    .min_inner_size(960.0, 600.0)
                    .center()
                    .initialization_script(&boot_script)
                    .data_directory(boot::webview2_data_dir(&data_dir))
                    .additional_browser_args(boot::WEBVIEW2_BROWSER_ARGS);

            if let (Some(x), Some(y)) = (window_state.x, window_state.y) {
                builder = builder.position(f64::from(x), f64::from(y));
            }

            let window = builder.build()?;
            if window_state.maximized {
                let _ = window.maximize();
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to start the FreeSpace desktop application");
}

/// Selects the WebView2 runtime before any webview exists.
///
/// A portable build ships an extracted fixed runtime in `webview2`; an
/// installed build uses the machine-wide Evergreen runtime.
fn select_webview2_runtime() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let Some(directory) = exe.parent() else {
        return;
    };

    boot::apply_fixed_runtime(directory);
}

/// Installs the local tracing subscriber.
///
/// Logs stay on the local machine; no remote exporter or file content is ever
/// written.
fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();
}

#[cfg(test)]
mod tests {
    use super::{commands, security};

    #[test]
    fn every_advertised_command_is_implemented() {
        let capabilities = security::capabilities();
        assert_eq!(capabilities.commands.len(), security::COMMANDS.len());
        assert!(!capabilities.commands.is_empty());
        assert!(capabilities.commands.contains(&"app_bootstrap".to_owned()));
        assert!(capabilities.commands.contains(&"tree_children".to_owned()));
    }

    #[test]
    fn event_registry_is_not_empty() {
        assert!(
            security::capabilities()
                .events
                .contains(&"scan-progress".to_owned())
        );
        let _ = commands::app::SETTINGS_SECTIONS;
    }
}
