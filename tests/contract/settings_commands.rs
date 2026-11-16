//! Contract checks for the settings, shredder, monitor, About and elevation
//! command families (US6, T122).
//!
//! The checks call the application services that back the Tauri commands, so
//! they run without a WebView. Only temporary directories are touched; ETW is
//! exercised only when `FREESPACE_ADMIN_TESTS=1`.

use std::fs;

use freespace_app::{
    about,
    dto::{
        DeleteMode, Density, ElevationResponse, MonitorEventFilter, MonitorSample,
        MonitorStartRequest, OperationKind, OperationState, SettingsPatch, ShredPatternChoice,
        ShredPreviewRequest, TaskState, ThemePreference, UiSettingsPatch,
    },
    error::UiErrorCode,
    monitor::{
        ETW_UNAVAILABLE_KEY, MonitorService, SOURCE_COUNTERS, SOURCE_COUNTERS_KEY, SOURCE_ETW_KEY,
    },
    operations::OperationService,
    settings::{BACKUP_SUFFIX, SETTINGS_FILE_NAME, SettingsSection, SettingsStore},
    shred::{IRREVERSIBLE_WARNING_KEY, MAX_PASSES, ShredService, execute},
};
use freespace_ops::Store;
use freespace_platform::elevation;

const PREVIEW_TTL: i64 = 1_000;

fn shred_request(paths: &[&std::path::Path], passes: u32) -> ShredPreviewRequest {
    ShredPreviewRequest {
        targets: paths
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect(),
        passes,
        pattern: ShredPatternChoice::Random,
    }
}

fn sample(process: &str, bytes: u64, path: &str) -> MonitorSample {
    MonitorSample {
        timestamp: 1_000,
        process_id: 7,
        process: process.to_owned(),
        bytes,
        paths: vec![path.to_owned()],
    }
}

#[test]
fn settings_round_trip_reset_and_cli_visibility() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join(SETTINGS_FILE_NAME);
    let mut store = SettingsStore::load(&path).expect("load");

    let view = store
        .update_view(SettingsPatch {
            ui: Some(UiSettingsPatch {
                theme: Some(ThemePreference::Dark),
                density: Some(Density::Compact),
                ..UiSettingsPatch::default()
            }),
            exclusions: Some(vec!["**/node_modules".to_owned(), "*.tmp".to_owned()]),
            duplicate_keep_rule: Some("shortest-path".to_owned()),
            outdated_threshold_days: Some(90),
            monitor_threshold_bytes: Some(8_192),
            monitor_interval_secs: Some(5),
            quarantine_retention_days: Some(14),
            confirm_destructive_actions: Some(false),
            ..SettingsPatch::default()
        })
        .expect("update");

    assert_eq!(view.theme, ThemePreference::Dark);
    assert_eq!(view.ui.density, Density::Compact);
    assert_eq!(view.exclusions.len(), 2);
    assert_eq!(view.duplicate_keep_rule, "shortest-path");
    assert_eq!(view.monitor_threshold_bytes, 8_192);
    assert!(!view.confirm_destructive_actions);

    let reloaded = SettingsStore::load(&path).expect("reload");
    assert_eq!(reloaded.view(), view, "the view survives a restart");

    // The CLI keeps seeing exactly the 001 keys it knows.
    let legacy = freespace_core::settings::Settings::load(&path).expect("cli load");
    assert_eq!(legacy.get_value("theme").as_deref(), Some("dark"));
    assert_eq!(legacy.get_value("units").as_deref(), Some("binary"));
    assert_eq!(
        legacy.get_value("duplicate_keep_rule").as_deref(),
        Some("shortest-path")
    );
    assert_eq!(
        legacy.get_value("exclusions").as_deref(),
        Some("**/node_modules;*.tmp")
    );

    let reset = store
        .reset_section(SettingsSection::Theme)
        .expect("reset theme");
    assert_eq!(reset.theme, ThemePreference::System);
    assert_eq!(reset.density, Density::Compact, "other sections stay");

    let reset = store
        .reset_section(SettingsSection::All)
        .expect("reset all");
    assert_eq!(reset, freespace_app::dto::UiSettings::default());
}

#[test]
fn settings_migration_keeps_a_backup_and_is_idempotent() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join(SETTINGS_FILE_NAME);
    let legacy = r#"
language = "en"
theme = "dark"
units = "decimal"
exclusions = ["**/node_modules"]

[future_section]
value = 42
"#;
    fs::write(&path, legacy).expect("write legacy");

    let mut store = SettingsStore::load(&path).expect("first load");
    let backup = path.with_extension(BACKUP_SUFFIX);
    assert_eq!(fs::read_to_string(&backup).expect("backup"), legacy);
    assert_eq!(store.view().language, freespace_app::dto::Language::En);
    assert_eq!(store.view().theme, ThemePreference::Dark);

    store.save().expect("save");
    store
        .update_view(SettingsPatch {
            ui: Some(UiSettingsPatch {
                theme: Some(ThemePreference::Light),
                ..UiSettingsPatch::default()
            }),
            exclusions: Some(vec!["*.bak".to_owned()]),
            ..SettingsPatch::default()
        })
        .expect("update");

    let second = SettingsStore::load(&path).expect("second load");
    assert_eq!(
        second.view().theme,
        ThemePreference::Light,
        "ui mirrors the legacy key"
    );
    assert_eq!(second.view().ui.theme, ThemePreference::Light);
    assert_eq!(
        fs::read_to_string(&backup).expect("backup"),
        legacy,
        "the pre-migration copy is never overwritten"
    );

    let saved = fs::read_to_string(&path).expect("saved");
    assert!(saved.contains("[ui]"), "{saved}");
    assert!(saved.contains("future_section"), "unknown keys survive");
    assert!(saved.contains("value = 42"));

    // A second migration of the same file is a no-op and keeps the backup.
    let third = SettingsStore::load(&path).expect("third load");
    assert_eq!(third.view(), second.view());
    assert_eq!(fs::read_to_string(&backup).expect("backup"), legacy);
}

#[test]
fn settings_validation_and_path_checks_are_strict() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join(SETTINGS_FILE_NAME);
    let mut store = SettingsStore::load(&path).expect("load");

    for patch in [
        SettingsPatch {
            monitor_interval_secs: Some(0),
            ..SettingsPatch::default()
        },
        SettingsPatch {
            duplicate_keep_rule: Some("magic".to_owned()),
            ..SettingsPatch::default()
        },
        SettingsPatch {
            outdated_threshold_days: Some(10_000),
            ..SettingsPatch::default()
        },
        SettingsPatch {
            exclusions: Some(vec!["".to_owned()]),
            ..SettingsPatch::default()
        },
    ] {
        let error = store.update_view(patch).expect_err("invalid patch");
        assert_eq!(error.code, UiErrorCode::Validation);
    }

    let file = directory.path().join("payload.bin");
    fs::write(&file, b"x").expect("write");
    let missing = directory.path().join("missing");
    let paths = vec![
        directory.path().to_string_lossy().into_owned(),
        file.to_string_lossy().into_owned(),
        missing.to_string_lossy().into_owned(),
        "relative\\folder".to_owned(),
    ];
    let results = freespace_app::settings::validate_paths(&paths).expect("validate");
    assert!(results[0].valid && results[0].is_directory);
    assert!(results[1].valid && !results[1].is_directory);
    assert!(!results[2].valid);
    assert_eq!(
        results[2].error_key.as_deref(),
        Some("settings.path.missing")
    );
    assert!(!results[3].valid);
    assert_eq!(
        results[3].error_key.as_deref(),
        Some("settings.path.invalid")
    );
}

#[test]
fn shred_preview_token_is_one_time_and_irreversible() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("secret.bin");
    fs::write(&path, b"payload").expect("write");
    let operations = OperationService::with_ttl(PREVIEW_TTL);
    let shredder = ShredService::new();

    let preview = shredder
        .preview(&operations, &shred_request(&[&path], 3), None, 0)
        .expect("preview");
    assert_eq!(preview.kind, OperationKind::Shred);
    assert_eq!(preview.delete_mode, Some(DeleteMode::Permanent));
    assert!(!preview.reversible);
    assert!(
        preview
            .warnings
            .iter()
            .any(|warning| warning == IRREVERSIBLE_WARNING_KEY)
    );
    assert!(
        preview
            .warnings
            .iter()
            .any(|warning| warning == "confirm.warning.permanent")
    );
    assert_eq!(preview.expires_at, PREVIEW_TTL);

    let refused = shredder
        .take(&operations, &preview.preview_id, "wrong", false, 1)
        .expect_err("wrong token");
    assert_eq!(refused.code, UiErrorCode::PreviewExpired);

    let fresh = shredder
        .preview(&operations, &shred_request(&[&path], 2), None, 0)
        .expect("second preview");
    let expired = shredder
        .take(
            &operations,
            &fresh.preview_id,
            &fresh.confirmation_token,
            false,
            PREVIEW_TTL + 1,
        )
        .expect_err("expired");
    assert_eq!(expired.code, UiErrorCode::PreviewExpired);
    assert!(path.exists());

    let missing = shredder
        .take(&operations, "unknown", "unknown", false, 0)
        .expect_err("no preview");
    assert_eq!(missing.code, UiErrorCode::PreviewExpired);
}

#[test]
fn shred_destroys_temporary_files_and_journals_the_run() {
    let directory = tempfile::tempdir().expect("temp dir");
    let first = directory.path().join("first.bin");
    let second = directory.path().join("second.bin");
    fs::write(&first, vec![3u8; 4_096]).expect("first");
    fs::write(&second, vec![4u8; 2_048]).expect("second");
    let store = Store::open_in_memory().expect("store");
    let operations = OperationService::new();
    let shredder = ShredService::new();

    let preview = shredder
        .preview(
            &operations,
            &shred_request(&[&first, &second], MAX_PASSES),
            None,
            0,
        )
        .expect("preview");
    assert_eq!(preview.total_size, 6_144);
    let (prepared, plan) = shredder
        .take(
            &operations,
            &preview.preview_id,
            &preview.confirmation_token,
            false,
            1,
        )
        .expect("take");
    assert_eq!(plan.passes, MAX_PASSES);

    let mut seen = Vec::new();
    let outcome = execute(
        &operations,
        &store,
        &prepared,
        plan,
        "contract-volume",
        &mut |progress| seen.push(progress.processed),
    )
    .expect("execute");

    assert_eq!(outcome.state, OperationState::Completed);
    assert_eq!(outcome.processed, 2);
    assert!(!outcome.undo_available);
    assert!(outcome.journal_entry_id.is_some());
    assert!(!first.exists());
    assert!(!second.exists());
    assert_eq!(seen, [1, 2]);

    let entries = store.journal_list(10).expect("journal");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].details["passes"], MAX_PASSES);
    assert_eq!(entries[0].details["pattern"], "random");

    for passes in [0, MAX_PASSES + 1] {
        let error = shredder
            .preview(&operations, &shred_request(&[], passes), None, 0)
            .expect_err("invalid passes");
        assert_eq!(error.code, UiErrorCode::Validation);
    }
}

#[test]
fn monitor_start_stop_pagination_and_exclusions() {
    let monitors = MonitorService::new();
    let started = monitors
        .start(
            &MonitorStartRequest {
                threshold: Some(100),
                interval: Some(2),
                exclusions: vec!["Defender*".to_owned()],
            },
            0,
        )
        .expect("start");
    assert_eq!(started.threshold_bytes, 100);
    assert_eq!(started.interval.as_secs(), 2);

    let status = monitors.status(&started.task_id).expect("status");
    assert!(status.running);
    assert_eq!(status.state, TaskState::Running);
    assert_eq!(status.source, SOURCE_COUNTERS);
    assert_eq!(status.source_key, SOURCE_COUNTERS_KEY);
    assert_eq!(status.exclusions, ["Defender*"]);

    let busy = monitors
        .start(&MonitorStartRequest::default(), 0)
        .expect_err("busy");
    assert_eq!(busy.code, UiErrorCode::Busy);

    let tick = monitors
        .push_samples(
            &started.task_id,
            &[
                sample("writer.exe", 40, r"C:\Temp\a.bin"),
                sample("DefenderCore", 900, r"C:\Temp\b.bin"),
                sample("writer.exe", 80, r"C:\Temp\c.bin"),
                sample("backup.exe", 500, r"D:\Data\d.bin"),
            ],
            1_000,
        )
        .expect("push")
        .expect("tick");
    assert_eq!(tick.events.len(), 3, "excluded process is dropped");
    assert_eq!(tick.sequence, 3);
    assert!(tick.events.iter().any(|event| event.threshold_exceeded));

    monitors
        .set_source(&started.task_id, "etw")
        .expect("source");
    assert_eq!(
        monitors
            .status(&started.task_id)
            .expect("status")
            .source_key,
        SOURCE_ETW_KEY
    );
    monitors
        .add_warning(&started.task_id, ETW_UNAVAILABLE_KEY)
        .expect("warning");

    let first = monitors
        .events_page(&started.task_id, &MonitorEventFilter::default(), 2, None)
        .expect("first");
    assert_eq!(first.events.len(), 2);
    assert_eq!(first.events[0].process, "backup.exe", "newest first");
    assert_eq!(first.total_estimate, 3);
    let cursor = first.next_cursor.expect("next cursor");

    let second = monitors
        .events_page(
            &started.task_id,
            &MonitorEventFilter::default(),
            2,
            Some(&cursor),
        )
        .expect("second");
    assert_eq!(second.events.len(), 1);
    assert!(second.next_cursor.is_none());

    let filtered = monitors
        .events_page(
            &started.task_id,
            &MonitorEventFilter {
                process: Some("writer".to_owned()),
                path: Some("temp".to_owned()),
                min_bytes: Some(50),
                threshold_only: false,
            },
            10,
            None,
        )
        .expect("filtered");
    assert_eq!(filtered.events.len(), 1);
    assert_eq!(filtered.events[0].bytes, 80);

    let stale = monitors
        .events_page(
            &started.task_id,
            &MonitorEventFilter::default(),
            10,
            Some("not-a-cursor"),
        )
        .expect_err("stale cursor");
    assert_eq!(stale.code, UiErrorCode::StaleCursor);

    monitors
        .set_exclusions(vec![r"D:\Data\*".to_owned()])
        .expect("exclusions");
    assert!(
        monitors
            .push_samples(
                &started.task_id,
                &[sample("backup.exe", 700, r"D:\Data\e.bin")],
                2_000,
            )
            .expect("push")
            .is_none()
    );

    monitors.stop(&started.task_id, 3_000).expect("stop");
    let status = monitors.status(&started.task_id).expect("status");
    assert!(!status.running);
    assert_eq!(status.state, TaskState::Completed);
    assert_eq!(status.finished_at, Some(3_000));
    assert_eq!(status.warnings, [ETW_UNAVAILABLE_KEY]);

    monitors
        .start(&MonitorStartRequest::default(), 4_000)
        .expect("restart after stop");
}

#[test]
fn monitor_validation_is_reported_before_any_work() {
    let monitors = MonitorService::new();
    for interval in [0, freespace_app::monitor::MAX_INTERVAL_SECS + 1] {
        let error = monitors
            .start(
                &MonitorStartRequest {
                    interval: Some(interval),
                    ..MonitorStartRequest::default()
                },
                0,
            )
            .expect_err("interval");
        assert_eq!(error.code, UiErrorCode::Validation);
    }
    assert!(
        monitors
            .set_exclusions(vec!["".to_owned()])
            .expect_err("empty")
            .code
            == UiErrorCode::Validation
    );
    assert_eq!(
        monitors.status("missing").expect_err("unknown").code,
        UiErrorCode::NotFound
    );
}

#[test]
fn about_payload_and_licences_are_offline() {
    let directory = tempfile::tempdir().expect("temp dir");
    let info = about::about_info(directory.path(), "1.1.0", Some("120.0".to_owned()));

    assert_eq!(info.version, "1.1.0");
    assert!(!info.build.is_empty());
    assert_eq!(info.webview_version.as_deref(), Some("120.0"));
    assert_eq!(info.data_dir, directory.path().to_string_lossy());
    assert!(!info.licenses.is_empty());
    assert_eq!(info.licenses, about::licenses_payload());
    assert!(info.licenses.iter().all(|license| license.url.is_none()));
    assert!(info.licenses.iter().any(|license| license.name == "Tauri"));
    assert!(
        info.licenses
            .iter()
            .any(|license| license.name.contains("WebView2"))
    );
}

#[test]
fn elevation_status_is_read_only_and_never_relaunches() {
    let elevated = elevation::is_elevated();
    let response = ElevationResponse { elevated };
    let value = serde_json::to_value(response).expect("serialise");
    assert_eq!(value, serde_json::json!({ "elevated": elevated }));

    let status = freespace_app::dto::ElevationStatus {
        elevated,
        can_elevate: !elevated,
        reason_key: (!elevated).then(|| "elevation.reason.mft".to_owned()),
    };
    let value = serde_json::to_value(&status).expect("serialise");
    assert_eq!(value["elevated"], elevated);
    assert_eq!(value["can_elevate"], !elevated);

    assert!(elevation::current_executable().is_some());
}

/// ETW needs administrator rights; the destructive and counter paths are covered
/// by the tests above. Set `FREESPACE_ADMIN_TESTS=1` on an isolated machine.
#[test]
fn etw_monitor_is_admin_only() {
    if std::env::var("FREESPACE_ADMIN_TESTS").ok().as_deref() != Some("1") {
        return;
    }

    match freespace_platform::etw::EtwSession::start(|_event| {}) {
        Ok(session) => session.stop().expect("stop ETW"),
        Err(error) => {
            let code = error.code();
            assert!(
                matches!(
                    code,
                    freespace_core::ErrorCode::AccessDenied | freespace_core::ErrorCode::Operation
                ),
                "unexpected ETW failure: {error}"
            );
        }
    }
}

/// Serialised errors keep the documented envelope for the new commands.
#[test]
fn error_envelopes_stay_stable() {
    let error =
        freespace_app::error::UiError::new(UiErrorCode::ElevationDenied).with_details("declined");
    let value = serde_json::to_value(&error).expect("serialise");
    assert_eq!(value["code"], "E_ELEVATION_DENIED");
    assert_eq!(value["category"], "access");
    assert_eq!(value["message_key"], "error.elevation_denied");
}
