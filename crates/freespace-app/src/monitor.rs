//! Write-monitoring sessions, events and exclusions (US6, T126).
//!
//! The policy — what to ignore, when the threshold is crossed and what history
//! to keep — stays in `freespace-analysis::monitor`. This module adds the task
//! lifecycle used by the interface: one running session at a time, a monotonic
//! sequence per task, a bounded pageable event list and the `monitor.source.*`
//! keys that explain whether ETW or the counter fallback is active.
//!
//! The platform worker lives in the desktop crate: it feeds [`MonitorSample`]s
//! into [`MonitorService::push_samples`] and emits `monitor-tick` at 1–2 Hz.

use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use freespace_analysis::monitor::{MonitorConfig, MonitorEvent, MonitorHistory};
use freespace_core::Timestamp;
use uuid::Uuid;

use crate::{
    dto::{
        MonitorEventFilter, MonitorEventPage, MonitorEventView, MonitorSample, MonitorStartRequest,
        MonitorStatus, MonitorTick, TaskState,
    },
    error::{UiError, UiErrorCode},
    validation::{MAX_EXCLUSION_CHARS, MAX_EXCLUSIONS, validate_page_size},
};

/// Source key used when the Kernel-File ETW session is active.
pub const SOURCE_ETW: &str = "etw";
/// Source key used when only process I/O counters are available.
pub const SOURCE_COUNTERS: &str = "counters";
/// Localisation key of the ETW source.
pub const SOURCE_ETW_KEY: &str = "monitor.source.etw";
/// Localisation key of the counter fallback.
pub const SOURCE_COUNTERS_KEY: &str = "monitor.source.counters";
/// Warning key emitted when ETW is unavailable and the fallback is used.
pub const ETW_UNAVAILABLE_KEY: &str = "monitor.fallback";
/// Warning key emitted when the counter fallback fails to poll.
pub const COUNTERS_UNAVAILABLE_KEY: &str = "monitor.warning.counters_unavailable";
/// Default payload size of `monitor_events_page`.
pub const DEFAULT_PAGE_SIZE: u16 = 500;
/// Smallest monitoring interval in seconds.
pub const MIN_INTERVAL_SECS: u64 = 1;
/// Largest monitoring interval in seconds.
pub const MAX_INTERVAL_SECS: u64 = 86_400;

/// Result of [`MonitorService::start`].
#[derive(Debug, Clone)]
pub struct MonitorStarted {
    /// Background task identifier.
    pub task_id: String,
    /// Cancellation flag observed by the platform worker.
    pub stop: Arc<AtomicBool>,
    /// Aggregation window used by the policy.
    pub interval: Duration,
    /// Active notification threshold.
    pub threshold_bytes: u64,
}

/// Values used when `monitor_start` does not override them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorDefaults {
    /// Notification threshold in bytes.
    pub threshold_bytes: u64,
    /// Aggregation window in seconds.
    pub interval_secs: u64,
    /// Exclusions from the persisted settings.
    pub exclusions: Vec<String>,
}

impl Default for MonitorDefaults {
    fn default() -> Self {
        let config = MonitorConfig::default();

        Self {
            threshold_bytes: config.threshold_bytes,
            interval_secs: config.interval.as_secs().max(1),
            exclusions: Vec::new(),
        }
    }
}

impl MonitorDefaults {
    /// Reads the monitor part of the 001 settings.
    #[must_use]
    pub fn from_settings(settings: &freespace_core::settings::Settings) -> Self {
        Self {
            threshold_bytes: settings.monitor_threshold.threshold_bytes,
            interval_secs: settings.monitor_threshold.interval_secs.max(1),
            exclusions: settings.exclusions.clone(),
        }
    }
}

#[derive(Debug)]
struct Session {
    task_id: String,
    config: MonitorConfig,
    history: MonitorHistory,
    events: VecDeque<MonitorEventView>,
    user_exclusions: Vec<String>,
    source: String,
    state: TaskState,
    started_at: i64,
    finished_at: Option<i64>,
    last_event_at: Option<i64>,
    sequence: u64,
    events_count: u64,
    warnings: Vec<String>,
    error: Option<UiError>,
    stop: Arc<AtomicBool>,
}

#[derive(Debug, Default)]
struct Inner {
    sessions: HashMap<String, Session>,
    exclusions: Vec<String>,
}

/// Monitor session registry shared by the commands and the worker.
#[derive(Debug, Default)]
pub struct MonitorService {
    inner: Mutex<Inner>,
}

impl MonitorService {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts a monitor session with the default policy.
    ///
    /// # Errors
    /// Returns `E_BUSY` when another session is still running and
    /// `E_VALIDATION` for a malformed interval or exclusion list.
    pub fn start(
        &self,
        request: &MonitorStartRequest,
        now_ms: i64,
    ) -> Result<MonitorStarted, UiError> {
        self.start_with_defaults(request, &MonitorDefaults::default(), now_ms)
    }

    /// Starts a monitor session, using the persisted settings as defaults.
    ///
    /// # Errors
    /// Returns `E_BUSY` when another session is still running and
    /// `E_VALIDATION` for a malformed interval or exclusion list.
    pub fn start_with_defaults(
        &self,
        request: &MonitorStartRequest,
        defaults: &MonitorDefaults,
        now_ms: i64,
    ) -> Result<MonitorStarted, UiError> {
        let exclusions = validate_exclusions(&request.exclusions)?;
        let interval_secs = match request.interval {
            Some(interval) if !(MIN_INTERVAL_SECS..=MAX_INTERVAL_SECS).contains(&interval) => {
                return Err(UiError::validation(format!(
                    "interval must be between {MIN_INTERVAL_SECS} and {MAX_INTERVAL_SECS} seconds"
                )));
            }
            Some(interval) => interval,
            None => defaults
                .interval_secs
                .clamp(MIN_INTERVAL_SECS, MAX_INTERVAL_SECS),
        };

        let mut inner = self.lock();
        if let Some(task_id) = inner
            .sessions
            .values()
            .find(|session| !session.state.is_finished())
            .map(|session| session.task_id.clone())
        {
            return Err(UiError::new(UiErrorCode::Busy)
                .with_details(format!("monitor task `{task_id}` is already running")));
        }

        let mut merged = inner.exclusions.clone();
        for exclusion in defaults.exclusions.iter().chain(&exclusions) {
            if !merged.contains(exclusion) {
                merged.push(exclusion.clone());
            }
        }

        let mut config = MonitorConfig {
            interval: Duration::from_secs(interval_secs),
            ..MonitorConfig::default()
        };
        config.threshold_bytes = request.threshold.unwrap_or(defaults.threshold_bytes);
        apply_exclusions(&mut config, &merged);

        let task_id = Uuid::new_v4().to_string();
        let stop = Arc::new(AtomicBool::new(false));
        let threshold_bytes = config.threshold_bytes;
        inner.sessions.insert(
            task_id.clone(),
            Session {
                task_id: task_id.clone(),
                history: MonitorHistory::new(
                    freespace_analysis::monitor::DEFAULT_HISTORY_CAPACITY,
                    &config,
                ),
                config,
                events: VecDeque::new(),
                user_exclusions: merged,
                source: SOURCE_COUNTERS.to_owned(),
                state: TaskState::Running,
                started_at: now_ms,
                finished_at: None,
                last_event_at: None,
                sequence: 0,
                events_count: 0,
                warnings: Vec::new(),
                error: None,
                stop: Arc::clone(&stop),
            },
        );

        Ok(MonitorStarted {
            task_id,
            stop,
            interval: Duration::from_secs(interval_secs),
            threshold_bytes,
        })
    }

    /// Records which source the worker managed to start.
    ///
    /// # Errors
    /// Returns `E_NOT_FOUND` for an unknown task and `E_VALIDATION` for an
    /// unknown source name.
    pub fn set_source(&self, task_id: &str, source: &str) -> Result<(), UiError> {
        if source != SOURCE_ETW && source != SOURCE_COUNTERS {
            return Err(UiError::validation(format!(
                "unknown monitor source `{source}`"
            )));
        }
        let mut inner = self.lock();
        let session = inner
            .sessions
            .get_mut(task_id)
            .ok_or_else(|| UiError::new(UiErrorCode::NotFound))?;
        session.source = source.to_owned();

        Ok(())
    }

    /// Adds a localisable warning to a session; duplicates are ignored.
    ///
    /// # Errors
    /// Returns `E_NOT_FOUND` for an unknown task.
    pub fn add_warning(&self, task_id: &str, key: &str) -> Result<(), UiError> {
        let mut inner = self.lock();
        let session = inner
            .sessions
            .get_mut(task_id)
            .ok_or_else(|| UiError::new(UiErrorCode::NotFound))?;
        if !session.warnings.iter().any(|warning| warning == key) {
            session.warnings.push(key.to_owned());
        }

        Ok(())
    }

    /// Marks the session as stopped and asks the worker to exit.
    ///
    /// # Errors
    /// Returns `E_NOT_FOUND` for an unknown task.
    pub fn stop(&self, task_id: &str, now_ms: i64) -> Result<(), UiError> {
        let mut inner = self.lock();
        let session = inner
            .sessions
            .get_mut(task_id)
            .ok_or_else(|| UiError::new(UiErrorCode::NotFound))?;
        session.stop.store(true, Ordering::Relaxed);
        if !session.state.is_finished() {
            session.state = TaskState::Completed;
            session.finished_at = Some(now_ms);
        }

        Ok(())
    }

    /// Stores an unrecoverable worker failure.
    ///
    /// # Errors
    /// Returns `E_NOT_FOUND` for an unknown task.
    pub fn fail(&self, task_id: &str, error: UiError, now_ms: i64) -> Result<(), UiError> {
        let mut inner = self.lock();
        let session = inner
            .sessions
            .get_mut(task_id)
            .ok_or_else(|| UiError::new(UiErrorCode::NotFound))?;
        session.stop.store(true, Ordering::Relaxed);
        session.state = TaskState::Failed;
        session.finished_at = Some(now_ms);
        session.error = Some(error);

        Ok(())
    }

    /// Applies policy, records accepted bursts and returns a tick payload.
    ///
    /// Excluded processes and paths are dropped by the same wildcard rules used
    /// by analysis. A tick is returned only when at least one burst was kept.
    ///
    /// # Errors
    /// Returns `E_NOT_FOUND` for an unknown task.
    pub fn push_samples(
        &self,
        task_id: &str,
        samples: &[MonitorSample],
        _now_ms: i64,
    ) -> Result<Option<MonitorTick>, UiError> {
        let mut inner = self.lock();
        let session = inner
            .sessions
            .get_mut(task_id)
            .ok_or_else(|| UiError::new(UiErrorCode::NotFound))?;
        if session.state.is_finished() {
            return Ok(None);
        }

        let mut accepted = Vec::new();
        for sample in samples {
            if sample.bytes == 0 {
                continue;
            }
            if session.config.excludes_process(&sample.process) {
                continue;
            }
            if !sample.paths.is_empty()
                && sample
                    .paths
                    .iter()
                    .all(|path| session.config.excludes_path(path))
            {
                continue;
            }

            let event = MonitorEvent {
                timestamp: Timestamp::from_millis(sample.timestamp),
                process_id: sample.process_id,
                process: sample.process.clone(),
                bytes: sample.bytes,
                paths: sample.paths.clone(),
                threshold_exceeded: false,
            };
            let notification = session.history.push(event, &session.config);
            session.sequence = session.sequence.saturating_add(1);
            session.events_count = session.events_count.saturating_add(1);
            session.last_event_at = Some(sample.timestamp);

            let view = MonitorEventView {
                sequence: session.sequence,
                timestamp: sample.timestamp,
                process_id: sample.process_id,
                process: sample.process.clone(),
                bytes: sample.bytes,
                paths: sample.paths.clone(),
                threshold_exceeded: notification.is_some(),
            };
            session.events.push_front(view.clone());
            while session.events.len() > freespace_analysis::monitor::DEFAULT_HISTORY_CAPACITY {
                session.events.pop_back();
            }
            accepted.push(view);
        }

        if accepted.is_empty() {
            return Ok(None);
        }

        Ok(Some(MonitorTick {
            sequence: session.sequence,
            events: accepted,
        }))
    }

    /// Returns the visible state of one monitor task.
    ///
    /// # Errors
    /// Returns `E_NOT_FOUND` for an unknown task.
    pub fn status(&self, task_id: &str) -> Result<MonitorStatus, UiError> {
        let inner = self.lock();
        let session = inner
            .sessions
            .get(task_id)
            .ok_or_else(|| UiError::new(UiErrorCode::NotFound))?;
        let source_key = match session.source.as_str() {
            SOURCE_ETW => SOURCE_ETW_KEY,
            _ => SOURCE_COUNTERS_KEY,
        };

        Ok(MonitorStatus {
            task_id: session.task_id.clone(),
            running: !session.state.is_finished(),
            state: session.state,
            source: session.source.clone(),
            source_key: source_key.to_owned(),
            threshold_bytes: session.config.threshold_bytes,
            interval_secs: session.config.interval.as_secs().max(1),
            exclusions: session.user_exclusions.clone(),
            events_count: session.events_count,
            bytes_total: session.history.total_bytes(),
            started_at: session.started_at,
            finished_at: session.finished_at,
            last_event_at: session.last_event_at,
            warnings: session.warnings.clone(),
            error: session.error.clone(),
        })
    }

    /// Returns one page of accepted events, newest first.
    ///
    /// # Errors
    /// Returns `E_NOT_FOUND` for an unknown task, `E_VALIDATION` for a bad page
    /// size and `E_STALE_CURSOR` when the cursor cannot be decoded.
    pub fn events_page(
        &self,
        task_id: &str,
        filter: &MonitorEventFilter,
        page_size: u16,
        cursor: Option<&str>,
    ) -> Result<MonitorEventPage, UiError> {
        validate_page_size(page_size)?;
        let inner = self.lock();
        let session = inner
            .sessions
            .get(task_id)
            .ok_or_else(|| UiError::new(UiErrorCode::NotFound))?;

        let matching: Vec<MonitorEventView> = session
            .events
            .iter()
            .filter(|event| event_matches(event, filter))
            .cloned()
            .collect();
        let offset = cursor
            .map(|cursor| cursor.parse::<usize>())
            .transpose()
            .map_err(|_| UiError::new(UiErrorCode::StaleCursor))?
            .unwrap_or(0);
        let end = offset
            .saturating_add(usize::from(page_size))
            .min(matching.len());
        let events = matching[offset.min(matching.len())..end].to_vec();
        let next_cursor = (end < matching.len()).then(|| end.to_string());

        Ok(MonitorEventPage {
            task_id: task_id.to_owned(),
            cursor: cursor.map(str::to_owned),
            next_cursor,
            events,
            total_estimate: matching.len() as u64,
        })
    }

    /// Replaces the exclusions used by future and running sessions.
    ///
    /// # Errors
    /// Returns `E_VALIDATION` for an empty, overlong or NUL-containing entry.
    pub fn set_exclusions(&self, exclusions: Vec<String>) -> Result<(), UiError> {
        let exclusions = validate_exclusions(&exclusions)?;
        let mut inner = self.lock();
        inner.exclusions = exclusions.clone();
        for session in inner.sessions.values_mut() {
            session.user_exclusions = exclusions.clone();
            reset_exclusions(&mut session.config);
            apply_exclusions(&mut session.config, &exclusions);
        }

        Ok(())
    }

    /// Current exclusions, used by a freshly started session.
    #[must_use]
    pub fn exclusions(&self) -> Vec<String> {
        self.lock().exclusions.clone()
    }

    /// Identifier of the running session, when any.
    #[must_use]
    pub fn active_task_id(&self) -> Option<String> {
        self.lock()
            .sessions
            .values()
            .find(|session| !session.state.is_finished())
            .map(|session| session.task_id.clone())
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

fn event_matches(event: &MonitorEventView, filter: &MonitorEventFilter) -> bool {
    if let Some(process) = &filter.process {
        let process = process.trim().to_lowercase();
        if !process.is_empty() && !event.process.to_lowercase().contains(&process) {
            return false;
        }
    }
    if let Some(path) = &filter.path {
        let path = path.trim().to_lowercase();
        if !path.is_empty()
            && !event
                .paths
                .iter()
                .any(|candidate| candidate.to_lowercase().contains(&path))
        {
            return false;
        }
    }
    if let Some(min_bytes) = filter.min_bytes {
        if event.bytes < min_bytes {
            return false;
        }
    }
    if filter.threshold_only && !event.threshold_exceeded {
        return false;
    }

    true
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

fn reset_exclusions(config: &mut MonitorConfig) {
    let defaults = MonitorConfig::default();
    config.excluded_processes = defaults.excluded_processes;
    config.excluded_paths = defaults.excluded_paths;
}

fn apply_exclusions(config: &mut MonitorConfig, exclusions: &[String]) {
    for exclusion in exclusions {
        if exclusion.contains(['\\', '/', ':']) {
            if !config.excluded_paths.contains(exclusion) {
                config.excluded_paths.push(exclusion.clone());
            }
        } else if !config.excluded_processes.contains(exclusion) {
            config.excluded_processes.push(exclusion.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        COUNTERS_UNAVAILABLE_KEY, ETW_UNAVAILABLE_KEY, MonitorService, SOURCE_COUNTERS,
        SOURCE_COUNTERS_KEY, SOURCE_ETW, SOURCE_ETW_KEY,
    };
    use crate::dto::{MonitorEventFilter, MonitorSample, MonitorStartRequest, TaskState};
    use crate::error::UiErrorCode;

    fn service() -> MonitorService {
        MonitorService::new()
    }

    fn sample(process: &str, bytes: u64, path: &str) -> MonitorSample {
        MonitorSample {
            timestamp: 1_000,
            process_id: 42,
            process: process.to_owned(),
            bytes,
            paths: vec![path.to_owned()],
        }
    }

    #[test]
    fn starts_one_session_at_a_time_and_reports_the_source() {
        let service = service();
        let started = service
            .start(&MonitorStartRequest::default(), 0)
            .expect("start");
        assert!(service.active_task_id().is_some());

        let busy = service
            .start(&MonitorStartRequest::default(), 0)
            .expect_err("busy");
        assert_eq!(busy.code, UiErrorCode::Busy);

        service
            .set_source(&started.task_id, SOURCE_ETW)
            .expect("source");
        let status = service.status(&started.task_id).expect("status");
        assert!(status.running);
        assert_eq!(status.source, SOURCE_ETW);
        assert_eq!(status.source_key, SOURCE_ETW_KEY);

        service
            .set_source(&started.task_id, SOURCE_COUNTERS)
            .expect("source");
        assert_eq!(
            service.status(&started.task_id).expect("status").source_key,
            SOURCE_COUNTERS_KEY
        );

        service.stop(&started.task_id, 10).expect("stop");
        assert!(started.stop.load(std::sync::atomic::Ordering::Relaxed));
        let status = service.status(&started.task_id).expect("status");
        assert!(!status.running);
        assert_eq!(status.state, TaskState::Completed);
        assert_eq!(status.finished_at, Some(10));
        assert!(service.active_task_id().is_none());

        service
            .start(&MonitorStartRequest::default(), 20)
            .expect("restart");
    }

    #[test]
    fn rejects_invalid_intervals_and_exclusions() {
        let service = service();
        for interval in [0, super::MAX_INTERVAL_SECS + 1] {
            let error = service
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

        let error = service
            .set_exclusions(vec!["".to_owned()])
            .expect_err("empty exclusion");
        assert_eq!(error.code, UiErrorCode::Validation);

        let error = service
            .set_exclusions(vec!["x".repeat(super::MAX_EXCLUSION_CHARS + 1)])
            .expect_err("long exclusion");
        assert_eq!(error.code, UiErrorCode::Validation);
    }

    #[test]
    fn filters_and_pages_events_newest_first() {
        let service = service();
        let started = service
            .start(
                &MonitorStartRequest {
                    threshold: Some(100),
                    interval: Some(2),
                    exclusions: vec!["Defender*".to_owned()],
                },
                0,
            )
            .expect("start");

        let samples = [
            sample("writer.exe", 40, r"C:\Temp\a.bin"),
            sample("DefenderCore", 900, r"C:\Temp\b.bin"),
            sample("writer.exe", 80, r"C:\Temp\c.bin"),
            sample("backup.exe", 500, r"D:\Data\d.bin"),
        ];
        let tick = service
            .push_samples(&started.task_id, &samples, 1_000)
            .expect("push")
            .expect("tick");
        assert_eq!(tick.events.len(), 3, "the excluded process is dropped");
        assert_eq!(tick.sequence, 3);
        assert!(tick.events.iter().any(|event| event.threshold_exceeded));

        let page = service
            .events_page(&started.task_id, &MonitorEventFilter::default(), 2, None)
            .expect("page");
        assert_eq!(page.events.len(), 2);
        assert_eq!(page.events[0].process, "backup.exe");
        assert_eq!(page.total_estimate, 3);
        let cursor = page.next_cursor.expect("next cursor");

        let second = service
            .events_page(
                &started.task_id,
                &MonitorEventFilter::default(),
                2,
                Some(&cursor),
            )
            .expect("second page");
        assert_eq!(second.events.len(), 1);
        assert!(second.next_cursor.is_none());

        let filtered = service
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

        let stale = service
            .events_page(
                &started.task_id,
                &MonitorEventFilter::default(),
                10,
                Some("not-a-cursor"),
            )
            .expect_err("stale");
        assert_eq!(stale.code, UiErrorCode::StaleCursor);
    }

    #[test]
    fn exclusions_apply_to_running_sessions_and_warnings_are_unique() {
        let service = service();
        let started = service
            .start(&MonitorStartRequest::default(), 0)
            .expect("start");
        service
            .set_exclusions(vec![r"C:\Windows\Temp\*".to_owned()])
            .expect("exclusions");

        assert_eq!(service.exclusions().len(), 1);
        let tick = service
            .push_samples(
                &started.task_id,
                &[sample("writer.exe", 10, r"C:\Windows\Temp\x.tmp")],
                1,
            )
            .expect("push");
        assert!(tick.is_none());

        service
            .add_warning(&started.task_id, ETW_UNAVAILABLE_KEY)
            .expect("warning");
        service
            .add_warning(&started.task_id, ETW_UNAVAILABLE_KEY)
            .expect("duplicate");
        service
            .add_warning(&started.task_id, COUNTERS_UNAVAILABLE_KEY)
            .expect("warning");
        let status = service.status(&started.task_id).expect("status");
        assert_eq!(status.warnings.len(), 2);
        assert_eq!(status.exclusions, [r"C:\Windows\Temp\*"]);
    }

    #[test]
    fn unknown_tasks_are_not_found() {
        let service = service();
        let error = service.status("missing").expect_err("status");
        assert_eq!(error.code, UiErrorCode::NotFound);
        assert_eq!(
            service
                .events_page("missing", &MonitorEventFilter::default(), 10, None)
                .expect_err("page")
                .code,
            UiErrorCode::NotFound
        );
        assert_eq!(
            service.stop("missing", 0).expect_err("stop").code,
            UiErrorCode::NotFound
        );
    }

    #[test]
    fn persisted_settings_supply_the_defaults() {
        let mut settings = freespace_core::settings::Settings::default();
        settings
            .set_value("monitor_threshold_bytes", "2048")
            .expect("threshold");
        settings
            .set_value("monitor_interval_secs", "7")
            .expect("interval");
        settings
            .set_value("exclusions", "Defender*")
            .expect("exclusions");
        let defaults = super::MonitorDefaults::from_settings(&settings);
        assert_eq!(defaults.threshold_bytes, 2_048);
        assert_eq!(defaults.interval_secs, 7);

        let service = service();
        let started = service
            .start_with_defaults(&MonitorStartRequest::default(), &defaults, 0)
            .expect("start");
        let status = service.status(&started.task_id).expect("status");
        assert_eq!(status.threshold_bytes, 2_048);
        assert_eq!(status.interval_secs, 7);
        assert_eq!(status.exclusions, ["Defender*"]);
    }

    #[test]
    fn failed_sessions_keep_the_envelope() {
        let service = service();
        let started = service
            .start(&MonitorStartRequest::default(), 0)
            .expect("start");
        service
            .fail(
                &started.task_id,
                crate::error::UiError::new(UiErrorCode::Internal),
                5,
            )
            .expect("fail");
        let status = service.status(&started.task_id).expect("status");
        assert_eq!(status.state, TaskState::Failed);
        assert!(status.error.is_some());
        assert!(!status.running);
    }
}
