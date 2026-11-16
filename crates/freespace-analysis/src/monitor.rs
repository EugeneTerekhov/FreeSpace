//! Write-monitor aggregation: thresholds, exclusions, history and notifications
//! (US20, FR-082…FR-085).
//!
//! The monitor receives raw bursts either from the ETW consumer (process and
//! path) or from the process-counter fallback (process and bytes, no path). Both
//! are normalised into [`MonitorEvent`]s; this module keeps the policy — what to
//! ignore, when to notify and what history to retain — independent of the
//! operating-system source so it can be tested deterministically.

use std::{
    collections::{HashMap, VecDeque},
    time::Duration,
};

use freespace_core::{
    Timestamp,
    settings::{MonitorThreshold, Settings},
};

/// Default number of events retained in memory.
pub const DEFAULT_HISTORY_CAPACITY: usize = 500;

/// Policy applied to incoming write events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorConfig {
    /// Bytes written within [`Self::interval`] that trigger a notification.
    pub threshold_bytes: u64,
    /// Aggregation window used for the threshold.
    pub interval: Duration,
    /// Process names (wildcards allowed) that are ignored.
    pub excluded_processes: Vec<String>,
    /// Path masks (wildcards allowed) that are ignored.
    pub excluded_paths: Vec<String>,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        let MonitorThreshold {
            threshold_bytes,
            interval_secs,
        } = MonitorThreshold::default();

        Self {
            threshold_bytes,
            interval: Duration::from_secs(interval_secs),
            excluded_processes: vec![
                "System".to_owned(),
                "Registry".to_owned(),
                "Memory Compression".to_owned(),
            ],
            excluded_paths: Vec::new(),
        }
    }
}

impl MonitorConfig {
    /// Builds the policy from user settings; the configured exclusions apply to
    /// both process names and paths.
    #[must_use]
    pub fn from_settings(settings: &Settings) -> Self {
        let mut config = Self {
            threshold_bytes: settings.monitor_threshold.threshold_bytes,
            interval: Duration::from_secs(settings.monitor_threshold.interval_secs.max(1)),
            excluded_processes: Self::default().excluded_processes,
            excluded_paths: Vec::new(),
        };
        for exclusion in &settings.exclusions {
            if exclusion.contains(['\\', '/', ':']) {
                config.excluded_paths.push(exclusion.clone());
            } else {
                config.excluded_processes.push(exclusion.clone());
            }
        }

        config
    }

    /// Returns `true` when the process is excluded.
    #[must_use]
    pub fn excludes_process(&self, process: &str) -> bool {
        self.excluded_processes
            .iter()
            .any(|pattern| wildcard_match(pattern, process))
    }

    /// Returns `true` when the path is excluded.
    #[must_use]
    pub fn excludes_path(&self, path: &str) -> bool {
        self.excluded_paths
            .iter()
            .any(|pattern| wildcard_match(pattern, path))
    }
}

/// One write burst attributed to a process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorEvent {
    /// When the burst was observed.
    pub timestamp: Timestamp,
    /// Process identifier.
    pub process_id: u32,
    /// Executable name.
    pub process: String,
    /// Bytes written.
    pub bytes: u64,
    /// Paths touched by the burst, when the source reports them.
    pub paths: Vec<String>,
    /// Whether the burst alone already exceeded the threshold.
    pub threshold_exceeded: bool,
}

/// Notification raised when the configured threshold is crossed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Notification {
    /// Bytes written within the window.
    pub bytes: u64,
    /// Window length in milliseconds.
    pub window_ms: i64,
    /// Threshold that was crossed.
    pub threshold_bytes: u64,
}

/// Totals of one process over the retained history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessSummary {
    /// Process identifier.
    pub process_id: u32,
    /// Executable name.
    pub process: String,
    /// Total bytes written.
    pub bytes: u64,
    /// Number of bursts.
    pub events: u64,
    /// Most recently touched path, when known.
    pub last_path: Option<String>,
}

/// Bounded event history with a sliding threshold window.
#[derive(Debug)]
pub struct MonitorHistory {
    capacity: usize,
    events: VecDeque<MonitorEvent>,
    window: VecDeque<(i64, u64)>,
    totals: HashMap<u32, (String, u64, u64, Option<String>)>,
    window_ms: i64,
    threshold_bytes: u64,
}

impl MonitorHistory {
    /// Creates a history with the given capacity and policy.
    #[must_use]
    pub fn new(capacity: usize, config: &MonitorConfig) -> Self {
        Self {
            capacity: capacity.max(1),
            events: VecDeque::new(),
            window: VecDeque::new(),
            totals: HashMap::new(),
            window_ms: i64::try_from(config.interval.as_millis()).unwrap_or(i64::MAX),
            threshold_bytes: config.threshold_bytes,
        }
    }

    /// Applies exclusions, stores the event and returns a notification when the
    /// threshold is crossed.
    ///
    /// Excluded processes and excluded paths are dropped; an event whose paths
    /// are all excluded counts as excluded.
    pub fn push(&mut self, event: MonitorEvent, config: &MonitorConfig) -> Option<Notification> {
        if config.excludes_process(&event.process) {
            return None;
        }
        if !event.paths.is_empty() && event.paths.iter().all(|path| config.excludes_path(path)) {
            return None;
        }
        if event.bytes == 0 {
            return None;
        }

        let timestamp = event.timestamp.as_millis();
        self.window.push_back((timestamp, event.bytes));
        let cutoff = timestamp.saturating_sub(self.window_ms.max(1));
        while self
            .window
            .front()
            .is_some_and(|(recorded, _)| *recorded < cutoff)
        {
            self.window.pop_front();
        }
        let window_bytes = self
            .window
            .iter()
            .fold(0u64, |total, (_, bytes)| total.saturating_add(*bytes));

        let summary = self
            .totals
            .entry(event.process_id)
            .or_insert_with(|| (event.process.clone(), 0, 0, None));
        summary.0.clone_from(&event.process);
        summary.1 = summary.1.saturating_add(event.bytes);
        summary.2 = summary.2.saturating_add(1);
        if let Some(path) = event.paths.last() {
            if summary.3.as_ref() != Some(path) {
                summary.3 = Some(path.clone());
            }
        }

        self.events.push_back(event);
        while self.events.len() > self.capacity {
            self.events.pop_front();
        }

        (self.threshold_bytes > 0 && window_bytes >= self.threshold_bytes).then_some(Notification {
            bytes: window_bytes,
            window_ms: self.window_ms,
            threshold_bytes: self.threshold_bytes,
        })
    }

    /// Retained events, oldest first.
    #[must_use]
    pub fn events(&self) -> &VecDeque<MonitorEvent> {
        &self.events
    }

    /// The most recent `limit` events, newest first.
    #[must_use]
    pub fn recent(&self, limit: usize) -> Vec<&MonitorEvent> {
        self.events.iter().rev().take(limit).collect()
    }

    /// Per-process totals, largest first.
    #[must_use]
    pub fn summaries(&self) -> Vec<ProcessSummary> {
        let mut summaries: Vec<ProcessSummary> = self
            .totals
            .iter()
            .map(
                |(process_id, (process, bytes, events, last_path))| ProcessSummary {
                    process_id: *process_id,
                    process: process.clone(),
                    bytes: *bytes,
                    events: *events,
                    last_path: last_path.clone(),
                },
            )
            .collect();
        summaries.sort_by(|left, right| {
            right
                .bytes
                .cmp(&left.bytes)
                .then_with(|| left.process.cmp(&right.process))
        });

        summaries
    }

    /// Total bytes in the retained history.
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.totals
            .values()
            .fold(0u64, |total, (_, bytes, _, _)| total.saturating_add(*bytes))
    }

    /// Drops every event and total.
    pub fn clear(&mut self) {
        self.events.clear();
        self.window.clear();
        self.totals.clear();
    }
}

/// Case-insensitive wildcard match supporting `*` and `?`.
#[must_use]
pub fn wildcard_match(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.to_lowercase().chars().collect();
    let text: Vec<char> = text.to_lowercase().chars().collect();
    let (mut pattern_index, mut text_index) = (0usize, 0usize);
    let mut backtrack: Option<(usize, usize)> = None;

    while text_index < text.len() {
        if pattern_index < pattern.len()
            && (pattern[pattern_index] == '?' || pattern[pattern_index] == text[text_index])
        {
            pattern_index += 1;
            text_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == '*' {
            backtrack = Some((pattern_index, text_index));
            pattern_index += 1;
        } else if let Some((star, matched)) = backtrack {
            pattern_index = star + 1;
            text_index = matched + 1;
            backtrack = Some((star, text_index));
        } else {
            return false;
        }
    }

    pattern[pattern_index..]
        .iter()
        .all(|character| *character == '*')
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use freespace_core::Timestamp;

    use super::{MonitorConfig, MonitorEvent, MonitorHistory, ProcessSummary, wildcard_match};

    fn config() -> MonitorConfig {
        MonitorConfig {
            threshold_bytes: 1_000,
            interval: Duration::from_secs(10),
            excluded_processes: vec!["System".to_owned(), "defender*".to_owned()],
            excluded_paths: vec![r"C:\Windows\Temp\*".to_owned()],
        }
    }

    fn event(millis: i64, pid: u32, process: &str, bytes: u64, path: &str) -> MonitorEvent {
        MonitorEvent {
            timestamp: Timestamp::from_millis(millis),
            process_id: pid,
            process: process.to_owned(),
            bytes,
            paths: vec![path.to_owned()],
            threshold_exceeded: false,
        }
    }

    #[test]
    fn matches_wildcards_case_insensitively() {
        assert!(wildcard_match("defender*", "DefenderCore"));
        assert!(wildcard_match("*\\Temp\\*", r"c:\windows\TEMP\a.tmp"));
        assert!(wildcard_match("a?c", "AbC"));
        assert!(!wildcard_match("a?c", "abbc"));
        assert!(!wildcard_match("defender*", "System"));
        assert!(wildcard_match("*", ""));
    }

    #[test]
    fn drops_excluded_sources() {
        let config = config();
        let mut history = MonitorHistory::new(10, &config);

        assert!(
            history
                .push(event(0, 1, "System", 5_000, r"C:\data.bin"), &config)
                .is_none()
        );
        assert!(
            history
                .push(event(0, 2, "DefenderCore", 5_000, r"C:\data.bin"), &config)
                .is_none()
        );
        assert!(
            history
                .push(
                    event(0, 3, "writer", 5_000, r"C:\Windows\Temp\a.tmp"),
                    &config
                )
                .is_none()
        );
        assert!(history.events().is_empty());

        assert!(
            history
                .push(event(0, 4, "writer", 5_000, r"C:\data.bin"), &config)
                .is_some(),
            "the amount crosses the threshold"
        );
    }

    #[test]
    fn notifies_when_the_window_crosses_the_threshold() {
        let config = config();
        let mut history = MonitorHistory::new(10, &config);

        assert!(
            history
                .push(event(0, 1, "writer", 400, r"C:\a"), &config)
                .is_none()
        );
        let notification = history
            .push(event(1_000, 1, "writer", 700, r"C:\a"), &config)
            .expect("threshold crossed");

        assert_eq!(notification.bytes, 1_100);
        assert_eq!(notification.threshold_bytes, 1_000);
    }

    #[test]
    fn forgets_bytes_outside_the_window() {
        let config = config();
        let mut history = MonitorHistory::new(10, &config);

        assert!(
            history
                .push(event(0, 1, "writer", 900, r"C:\a"), &config)
                .is_none()
        );
        assert!(
            history
                .push(event(11_000, 1, "writer", 900, r"C:\a"), &config)
                .is_none(),
            "the first burst aged out of the ten-second window"
        );
    }

    #[test]
    fn keeps_history_bounded_and_summarises_processes() {
        let config = config();
        let mut history = MonitorHistory::new(2, &config);

        history.push(event(0, 1, "alpha", 10, r"C:\a"), &config);
        history.push(event(1_000, 2, "beta", 30, r"C:\b"), &config);
        history.push(event(2_000, 2, "beta", 20, r"C:\c"), &config);

        assert_eq!(history.events().len(), 2);
        assert_eq!(history.recent(1)[0].bytes, 20);
        assert_eq!(history.total_bytes(), 60);
        assert_eq!(
            history.summaries(),
            vec![
                ProcessSummary {
                    process_id: 2,
                    process: "beta".to_owned(),
                    bytes: 50,
                    events: 2,
                    last_path: Some(r"C:\c".to_owned()),
                },
                ProcessSummary {
                    process_id: 1,
                    process: "alpha".to_owned(),
                    bytes: 10,
                    events: 1,
                    last_path: Some(r"C:\a".to_owned()),
                },
            ]
        );

        history.clear();
        assert!(history.events().is_empty());
        assert_eq!(history.total_bytes(), 0);
    }

    #[test]
    fn derives_configuration_from_settings() {
        let mut settings = freespace_core::settings::Settings::default();
        settings
            .set_value("monitor_threshold_bytes", "4096")
            .expect("threshold");
        settings
            .set_value("monitor_interval_secs", "5")
            .expect("interval");
        settings
            .set_value("exclusions", r"C:\Windows\Temp\*;Defender*")
            .expect("exclusions");

        let config = MonitorConfig::from_settings(&settings);

        assert_eq!(config.threshold_bytes, 4_096);
        assert_eq!(config.interval, Duration::from_secs(5));
        assert!(config.excludes_path(r"C:\Windows\Temp\a.tmp"));
        assert!(config.excludes_process("DefenderCore"));
        assert!(!config.excludes_process("explorer"));
    }
}
