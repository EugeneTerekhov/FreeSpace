//! Event envelopes, payloads and the 10–20 Hz throttle.
//!
//! Events carry progress and invalidation only; data is always requested through
//! a command. `sequence` grows monotonically per task, so the client can drop
//! duplicates and out-of-order deliveries.

use std::time::{Duration, Instant};

use serde::Serialize;

use freespace_app::dto::{Task, TaskProgress, TaskState, UiSettings};
use freespace_app::error::UiError;
use freespace_core::time::Timestamp;

/// Version of the event envelope.
pub const EVENT_VERSION: u32 = 1;

/// Events the interface may listen to.
pub const EVENT_NAMES: [&str; 11] = [
    "duplicates-completed",
    "duplicates-progress",
    "monitor-tick",
    "navigation-request",
    "operation-completed",
    "operation-progress",
    "scan-completed",
    "scan-progress",
    "settings-changed",
    "task-queue-changed",
    "volume-changed",
];

/// Common event envelope.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EventEnvelope<T> {
    /// Envelope version.
    pub version: u32,
    /// kebab-case event name.
    pub event: &'static str,
    /// Monotonic sequence inside the task.
    pub sequence: u64,
    /// Related task, when any.
    pub task_id: Option<String>,
    /// Emission time in ISO 8601.
    pub emitted_at: String,
    /// Event-specific body.
    pub payload: T,
}

impl<T> EventEnvelope<T> {
    /// Builds an envelope with the current timestamp.
    #[must_use]
    pub fn new(event: &'static str, sequence: u64, task_id: Option<String>, payload: T) -> Self {
        Self {
            version: EVENT_VERSION,
            event,
            sequence,
            task_id,
            emitted_at: Timestamp::now().format_iso8601(),
            payload,
        }
    }
}

/// Counts carried by the progress event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ProgressCounts {
    /// Objects processed.
    pub processed: u64,
    /// Objects in total, when known.
    pub total: Option<u64>,
}

/// Payload of `scan-progress`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScanProgressEvent {
    /// Processed counts.
    pub progress: ProgressCounts,
    /// Localisable phase key.
    pub phase_key: String,
    /// Current object.
    pub current_path: Option<String>,
    /// Discovered bytes.
    pub bytes: u64,
    /// Throughput in bytes per second.
    pub bytes_per_second: Option<u64>,
}

/// Summary attached to `scan-completed`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScanSummary {
    /// Produced nodes.
    pub nodes_count: u64,
    /// Method that ran; `mixed` when roots used different methods.
    pub method: String,
    /// Method of each root, in request order.
    pub methods: Vec<String>,
    /// Analysed roots.
    pub roots: Vec<String>,
    /// Mount points of the analysed volumes.
    pub volumes: Vec<String>,
    /// User-facing warnings.
    pub warnings: Vec<String>,
    /// Non-fatal issues.
    pub issues_count: u64,
    /// Elapsed milliseconds.
    pub elapsed_ms: u64,
}

/// Payload of `scan-completed`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScanCompletedEvent {
    /// Task identifier.
    pub task_id: String,
    /// Terminal state.
    pub state: TaskState,
    /// Generation of the produced snapshot.
    pub generation: u64,
    /// Summary when the scan produced data.
    pub summary: Option<ScanSummary>,
    /// Failure or partial-success envelope.
    pub error: Option<UiError>,
}

/// Payload of `task-queue-changed`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TaskQueueEvent {
    /// Every visible task.
    pub tasks: Vec<Task>,
}

/// Payload of `settings-changed`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SettingsChangedEvent {
    /// Updated interface settings.
    pub settings: UiSettings,
}

/// Payload of `navigation-request`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NavigationEvent {
    /// Target screen.
    pub screen: String,
    /// Node to select.
    pub node_id: Option<u32>,
    /// Path to reveal.
    pub path: Option<String>,
}

/// Payload of `volume-changed`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VolumeChangedEvent {
    /// Volumes after the change.
    pub volumes: Vec<freespace_app::dto::VolumeSummary>,
}

/// Payload of `duplicates-progress`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DuplicateProgressEvent {
    /// Processed counts.
    pub progress: ProgressCounts,
    /// Localisable phase key.
    pub phase_key: String,
}

/// Payload of `duplicates-completed`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DuplicateCompletedEvent {
    /// Task identifier.
    pub task_id: String,
    /// Terminal state.
    pub state: TaskState,
    /// Number of groups found.
    pub groups_count: u64,
    /// Total reclaimable bytes.
    pub reclaimable_bytes: u64,
    /// Failure envelope.
    pub error: Option<UiError>,
}

/// Payload of `operation-progress`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OperationProgressEvent {
    /// Current progress.
    #[serde(flatten)]
    pub progress: freespace_app::dto::OperationProgress,
}

/// Payload of `operation-completed`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OperationCompletedEvent {
    /// Operation identifier.
    pub operation_id: String,
    /// Terminal state.
    pub state: freespace_app::dto::OperationState,
    /// Journal entry, when written.
    pub journal_entry_id: Option<String>,
    /// True when undo or restore is available.
    pub undo_available: bool,
    /// Failure envelope.
    pub error: Option<UiError>,
}

/// Payload of `monitor-tick`.
///
/// The monitor emits at 1–2 Hz. `sequence` grows monotonically inside the task;
/// the client drops ticks whose sequence is not greater than the last seen one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MonitorTickEvent {
    /// Monitor task identifier.
    pub task_id: String,
    /// Monotonic sequence inside the task.
    pub sequence: u64,
    /// Events accepted since the previous tick, oldest first.
    pub events: Vec<freespace_app::dto::MonitorEventView>,
}

/// Throttle that limits an event family to a fixed rate.
#[derive(Debug, Clone)]
pub struct EventThrottle {
    interval: Duration,
    last: Option<Instant>,
}

impl EventThrottle {
    /// Creates a throttle at the default 20 Hz.
    #[must_use]
    pub fn new() -> Self {
        Self::with_interval(Duration::from_millis(50))
    }

    /// Creates a throttle with an explicit interval.
    #[must_use]
    pub const fn with_interval(interval: Duration) -> Self {
        Self {
            interval,
            last: None,
        }
    }

    /// Returns `true` (and records the instant) when the caller may emit.
    pub fn allow(&mut self, now: Instant) -> bool {
        match self.last {
            Some(last) if now.duration_since(last) < self.interval => false,
            Some(_) | None => {
                self.last = Some(now);
                true
            }
        }
    }
}

impl Default for EventThrottle {
    fn default() -> Self {
        Self::new()
    }
}

/// Monotonic per-task sequence.
#[derive(Debug, Default, Clone, Copy)]
pub struct Sequence(u64);

impl Sequence {
    /// Returns the next value.
    pub fn advance(&mut self) -> u64 {
        self.0 = self.0.saturating_add(1);
        self.0
    }

    /// Last issued value.
    #[must_use]
    pub const fn current(self) -> u64 {
        self.0
    }
}

/// Converts scan progress into the wire shape.
#[must_use]
pub fn scan_progress_event(
    progress: &freespace_scan::session::ScanProgress,
    elapsed: Duration,
    previous_bytes: u64,
) -> (ScanProgressEvent, u64) {
    let bytes = progress.bytes;
    let per_second = if elapsed.is_zero() {
        None
    } else {
        Some(((bytes.saturating_sub(previous_bytes)) as f64 / elapsed.as_secs_f64()) as u64)
    };

    (
        ScanProgressEvent {
            progress: ProgressCounts {
                processed: progress.nodes,
                total: None,
            },
            phase_key: format!("scan.phase.{}", progress.phase.as_str()),
            current_path: (!progress.current.as_os_str().is_empty())
                .then(|| freespace_platform::paths::display(&progress.current)),
            bytes,
            bytes_per_second: per_second,
        },
        bytes,
    )
}

/// Describes a task for the shell status bar.
#[must_use]
pub fn task_progress(
    progress: &freespace_scan::session::ScanProgress,
    started: Instant,
) -> TaskProgress {
    let elapsed = started.elapsed();
    let per_second = if elapsed.is_zero() {
        None
    } else {
        Some((progress.nodes as f64 / elapsed.as_secs_f64()) as u64)
    };

    TaskProgress {
        processed: progress.nodes,
        total: None,
        current_path: (!progress.current.as_os_str().is_empty())
            .then(|| freespace_platform::paths::display(&progress.current)),
        bytes_per_second: None,
        items_per_second: per_second,
        phase_key: format!("scan.phase.{}", progress.phase.as_str()),
        updated_at: Timestamp::now().as_millis(),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{
        EVENT_NAMES, EventEnvelope, EventThrottle, ProgressCounts, ScanProgressEvent, Sequence,
    };

    #[test]
    fn envelopes_serialise_the_documented_shape() {
        let envelope = EventEnvelope::new(
            "scan-progress",
            42,
            Some("task-1".to_owned()),
            ScanProgressEvent {
                progress: ProgressCounts {
                    processed: 1_000,
                    total: Some(100_000),
                },
                phase_key: "scan.phase.enumerating".to_owned(),
                current_path: Some(r"C:\Windows".to_owned()),
                bytes: 5_000,
                bytes_per_second: Some(1_000),
            },
        );
        let value = serde_json::to_value(&envelope).expect("serialise");

        assert_eq!(value["version"], 1);
        assert_eq!(value["event"], "scan-progress");
        assert_eq!(value["sequence"], 42);
        assert_eq!(value["task_id"], "task-1");
        assert!(
            value["emitted_at"]
                .as_str()
                .expect("timestamp")
                .ends_with('Z')
        );
        assert_eq!(value["payload"]["progress"]["processed"], 1_000);
        assert_eq!(value["payload"]["phase_key"], "scan.phase.enumerating");
    }

    #[test]
    fn throttle_limits_the_rate() {
        let mut throttle = EventThrottle::with_interval(Duration::from_millis(50));
        let start = Instant::now();

        assert!(throttle.allow(start));
        assert!(!throttle.allow(start + Duration::from_millis(10)));
        assert!(!throttle.allow(start + Duration::from_millis(49)));
        assert!(throttle.allow(start + Duration::from_millis(50)));
        assert!(throttle.allow(start + Duration::from_millis(200)));
    }

    #[test]
    fn sequence_is_monotonic() {
        let mut sequence = Sequence::default();
        assert_eq!(sequence.advance(), 1);
        assert_eq!(sequence.advance(), 2);
        assert_eq!(sequence.advance(), 3);
        assert_eq!(sequence.current(), 3);
    }

    #[test]
    fn exposes_every_documented_event_once() {
        let mut names = EVENT_NAMES.to_vec();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), EVENT_NAMES.len());
        for name in EVENT_NAMES {
            assert!(
                name.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
                "{name}"
            );
        }
        assert!(EVENT_NAMES.contains(&"monitor-tick"));
    }

    #[test]
    fn monitor_tick_carries_task_sequence_and_events() {
        let envelope = EventEnvelope::new(
            "monitor-tick",
            8,
            Some("task-7".to_owned()),
            super::MonitorTickEvent {
                task_id: "task-7".to_owned(),
                sequence: 3,
                events: vec![freespace_app::dto::MonitorEventView {
                    sequence: 3,
                    timestamp: 1_000,
                    process_id: 42,
                    process: "writer.exe".to_owned(),
                    bytes: 4_096,
                    paths: vec![r"C:\Temp\a.bin".to_owned()],
                    threshold_exceeded: true,
                }],
            },
        );
        let value = serde_json::to_value(&envelope).expect("serialise");

        assert_eq!(value["event"], "monitor-tick");
        assert_eq!(value["task_id"], "task-7");
        assert_eq!(value["payload"]["task_id"], "task-7");
        assert_eq!(value["payload"]["sequence"], 3);
        assert_eq!(value["payload"]["events"][0]["bytes"], 4_096);
        assert_eq!(value["payload"]["events"][0]["threshold_exceeded"], true);
    }
}
