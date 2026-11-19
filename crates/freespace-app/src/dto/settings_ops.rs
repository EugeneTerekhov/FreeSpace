//! Operation and settings command payloads (US6).
//!
//! The shapes follow `specs/002-native-ui-redesign/contracts/ipc.md` §9–§10.
//! The settings view keeps the 001 keys in their historical place and nests the
//! new `[ui]` section, so the CLI continues to see exactly the keys listed in
//! `freespace_core::settings::CONFIG_KEYS`.

use serde::{Deserialize, Serialize};

use crate::{
    dto::{Language, ThemePreference, UiSettings, UiSettingsPatch, UnitSystem},
    error::UiError,
};

/// Existing 001 settings plus the migrated `[ui]` section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsView {
    /// Interface language, mirrored from the legacy root key.
    pub language: Language,
    /// Colour theme, mirrored from the legacy root key.
    pub theme: ThemePreference,
    /// Unit system, mirrored from the legacy root key.
    pub units: UnitSystem,
    /// Path masks excluded from analysis and monitoring.
    pub exclusions: Vec<String>,
    /// Rule that selects the copy kept when deleting duplicates.
    pub duplicate_keep_rule: String,
    /// Age in days after which an object is considered stale.
    pub outdated_threshold_days: u32,
    /// Bytes written within the monitoring interval that raise a notification.
    pub monitor_threshold_bytes: u64,
    /// Write-monitoring interval in seconds.
    pub monitor_interval_secs: u64,
    /// Quarantine retention in days.
    pub quarantine_retention_days: u32,
    /// Explorer context-menu registration.
    pub context_menu_enabled: bool,
    /// Always ask before destructive actions.
    pub confirm_destructive_actions: bool,
    /// Interface section introduced by feature 002.
    pub ui: UiSettings,
}

/// Partial update of [`SettingsView`]; absent fields keep their value.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SettingsPatch {
    /// Partial update of the `[ui]` section.
    pub ui: Option<UiSettingsPatch>,
    /// Legacy language key; overrides `ui.language` when both are present.
    pub language: Option<Language>,
    /// Legacy theme key; overrides `ui.theme` when both are present.
    pub theme: Option<ThemePreference>,
    /// Legacy units key; overrides `ui.units` when both are present.
    pub units: Option<UnitSystem>,
    /// Replaces the exclusion list.
    pub exclusions: Option<Vec<String>>,
    /// Duplicate keep rule wire name.
    pub duplicate_keep_rule: Option<String>,
    /// Age in days after which an object is considered stale.
    pub outdated_threshold_days: Option<u32>,
    /// Monitoring notification threshold in bytes.
    pub monitor_threshold_bytes: Option<u64>,
    /// Monitoring interval in seconds.
    pub monitor_interval_secs: Option<u64>,
    /// Quarantine retention in days.
    pub quarantine_retention_days: Option<u32>,
    /// Explorer context-menu registration.
    pub context_menu_enabled: Option<bool>,
    /// Always ask before destructive actions.
    pub confirm_destructive_actions: Option<bool>,
}

/// Result of checking one user-supplied path.
///
/// The shape is owned by the inspection workstream (`dto::inspection`); the
/// settings screen reuses it so both screens describe a path identically.
pub use crate::dto::inspection::PathValidation;

/// Overwrite pattern exposed by the shredder screen.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShredPatternChoice {
    /// Overwrite with zero bytes.
    #[default]
    Zeros,
    /// Overwrite with cryptographically strong random data.
    Random,
}

impl ShredPatternChoice {
    /// Stable wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Zeros => "zeros",
            Self::Random => "random",
        }
    }

    /// Parses a wire name.
    #[must_use]
    pub fn parse_value(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "zeros" | "zero" => Some(Self::Zeros),
            "random" => Some(Self::Random),
            _ => None,
        }
    }
}

/// Input of `shred_preview`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShredPreviewRequest {
    /// Files to destroy; directories are rejected by the operator.
    pub targets: Vec<String>,
    /// Number of overwrite passes, 1..=35.
    #[serde(default = "default_shred_passes")]
    pub passes: u32,
    /// Overwrite pattern.
    #[serde(default)]
    pub pattern: ShredPatternChoice,
}

const fn default_shred_passes() -> u32 {
    1
}

impl Default for ShredPreviewRequest {
    fn default() -> Self {
        Self {
            targets: Vec::new(),
            passes: default_shred_passes(),
            pattern: ShredPatternChoice::Zeros,
        }
    }
}

/// Input of `monitor_start`.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MonitorStartRequest {
    /// Notification threshold in bytes; `None` uses the persisted setting.
    pub threshold: Option<u64>,
    /// Aggregation window in seconds; `None` uses the persisted setting.
    pub interval: Option<u64>,
    /// Process names or path masks added to the persisted exclusions.
    pub exclusions: Vec<String>,
}

/// One write burst shown by the monitor screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorEventView {
    /// Monotonic sequence inside the monitor task.
    pub sequence: u64,
    /// Observation time, Unix milliseconds.
    pub timestamp: i64,
    /// Process identifier.
    pub process_id: u32,
    /// Executable name, empty when it cannot be resolved.
    pub process: String,
    /// Bytes written by the burst.
    pub bytes: u64,
    /// Paths reported by ETW; empty for the counter fallback.
    pub paths: Vec<String>,
    /// Whether this burst crossed the notification threshold.
    pub threshold_exceeded: bool,
}

/// Filter applied by `monitor_events_page`.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MonitorEventFilter {
    /// Case-insensitive substring of the process name.
    pub process: Option<String>,
    /// Case-insensitive substring of a reported path.
    pub path: Option<String>,
    /// Lower bound of the written bytes.
    pub min_bytes: Option<u64>,
    /// Only bursts that crossed the threshold.
    pub threshold_only: bool,
}

/// One page of monitor events, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorEventPage {
    /// Monitor task the page belongs to.
    pub task_id: String,
    /// Cursor that produced this page.
    pub cursor: Option<String>,
    /// Cursor for the next page; `None` at the end.
    pub next_cursor: Option<String>,
    /// Events, at most `page_size`.
    pub events: Vec<MonitorEventView>,
    /// Rough total of matching events.
    pub total_estimate: u64,
}

/// Visible state of one monitor task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorStatus {
    /// Monitor task identifier.
    pub task_id: String,
    /// True while the worker is collecting events.
    pub running: bool,
    /// Lifecycle of the underlying task.
    pub state: crate::dto::TaskState,
    /// Active source: `etw` or `counters`.
    pub source: String,
    /// Localisable source key (`monitor.source.*`).
    pub source_key: String,
    /// Active notification threshold in bytes.
    pub threshold_bytes: u64,
    /// Active aggregation window in seconds.
    pub interval_secs: u64,
    /// Process names and path masks currently filtered out.
    pub exclusions: Vec<String>,
    /// Accepted bursts.
    pub events_count: u64,
    /// Total bytes of the accepted bursts.
    pub bytes_total: u64,
    /// Start time, Unix milliseconds.
    pub started_at: i64,
    /// Finish time, Unix milliseconds.
    pub finished_at: Option<i64>,
    /// Time of the latest accepted burst.
    pub last_event_at: Option<i64>,
    /// Localisable warning keys, for example a degraded source.
    pub warnings: Vec<String>,
    /// Failure envelope when the worker could not continue.
    pub error: Option<UiError>,
}

/// Payload of one `monitor-tick` batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorTick {
    /// Monotonic sequence inside the task.
    pub sequence: u64,
    /// Events accepted since the previous tick, oldest first.
    pub events: Vec<MonitorEventView>,
}

/// One sample handed to the monitor service by the platform worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorSample {
    /// Observation time, Unix milliseconds.
    pub timestamp: i64,
    /// Process identifier.
    pub process_id: u32,
    /// Executable name; empty when unknown.
    pub process: String,
    /// Bytes written by the burst.
    pub bytes: u64,
    /// Paths reported by ETW; empty for the counter fallback.
    pub paths: Vec<String>,
}

/// Input of `app_elevation_request`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElevationRequest {
    /// Localisable reason shown by the elevation screen.
    pub reason_key: String,
}

/// Output of `app_elevation_request`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElevationResponse {
    /// True when the current or a freshly started process is elevated.
    pub elevated: bool,
}

#[cfg(test)]
mod tests {
    use super::{
        ElevationRequest, ElevationResponse, MonitorEventPage, MonitorEventView,
        MonitorStartRequest, SettingsPatch, SettingsView, ShredPatternChoice, ShredPreviewRequest,
    };
    use crate::dto::{Language, ThemePreference, UiSettings, UiSettingsPatch};

    #[test]
    fn settings_view_carries_legacy_keys_and_the_ui_section() {
        let view = SettingsView {
            language: Language::En,
            theme: ThemePreference::Dark,
            units: crate::dto::UnitSystem::Decimal,
            exclusions: vec!["**/node_modules".to_owned()],
            duplicate_keep_rule: "newest".to_owned(),
            outdated_threshold_days: 90,
            monitor_threshold_bytes: 4_096,
            monitor_interval_secs: 15,
            quarantine_retention_days: 7,
            context_menu_enabled: false,
            confirm_destructive_actions: true,
            ui: UiSettings::default(),
        };
        let value = serde_json::to_value(&view).expect("serialise");

        assert_eq!(value["language"], "en");
        assert_eq!(value["theme"], "dark");
        assert_eq!(value["duplicate_keep_rule"], "newest");
        assert_eq!(value["ui"]["schema_version"], 1);
        assert!(value["ui"]["theme"].is_string());
    }

    #[test]
    fn patches_deserialise_partially() {
        let patch: SettingsPatch =
            serde_json::from_str(r#"{"ui":{"theme":"light"},"exclusions":["*.tmp"]}"#)
                .expect("patch");
        assert_eq!(patch.exclusions, Some(vec!["*.tmp".to_owned()]));
        assert_eq!(
            patch.ui.and_then(|ui: UiSettingsPatch| ui.theme),
            Some(ThemePreference::Light)
        );

        let shared: SettingsPatch =
            serde_json::from_str(r#"{"ui":{"theme":"light"},"theme":"dark"}"#).expect("both");
        assert_eq!(shared.theme, Some(ThemePreference::Dark));
    }

    #[test]
    fn shred_and_monitor_payloads_use_wire_names() {
        let request = ShredPreviewRequest {
            targets: vec![r"C:\Temp\secret.bin".to_owned()],
            passes: 3,
            pattern: ShredPatternChoice::Random,
        };
        let value = serde_json::to_value(&request).expect("serialise");
        assert_eq!(value["passes"], 3);
        assert_eq!(value["pattern"], "random");
        assert_eq!(
            ShredPatternChoice::parse_value("zeros"),
            Some(ShredPatternChoice::Zeros)
        );
        assert_eq!(ShredPatternChoice::parse_value("magic"), None);

        let monitor: MonitorStartRequest =
            serde_json::from_str(r#"{"threshold":1024,"interval":5}"#).expect("monitor");
        assert_eq!(monitor.threshold, Some(1_024));
        assert_eq!(monitor.interval, Some(5));
        assert!(monitor.exclusions.is_empty());
    }

    #[test]
    fn event_pages_round_trip() {
        let page = MonitorEventPage {
            task_id: "task-1".to_owned(),
            cursor: None,
            next_cursor: Some("2".to_owned()),
            events: vec![MonitorEventView {
                sequence: 1,
                timestamp: 10,
                process_id: 7,
                process: "writer.exe".to_owned(),
                bytes: 4_096,
                paths: vec![r"C:\Temp\a.bin".to_owned()],
                threshold_exceeded: false,
            }],
            total_estimate: 5,
        };
        let value = serde_json::to_value(&page).expect("serialise");
        assert_eq!(value["events"][0]["process_id"], 7);
        assert_eq!(value["next_cursor"], "2");
    }

    #[test]
    fn elevation_payloads_are_minimal() {
        let request: ElevationRequest =
            serde_json::from_str(r#"{"reason_key":"elevation.reason.mft"}"#).expect("request");
        assert_eq!(request.reason_key, "elevation.reason.mft");
        assert_eq!(
            serde_json::to_value(ElevationResponse { elevated: true }).expect("serialise"),
            serde_json::json!({ "elevated": true })
        );
    }
}
