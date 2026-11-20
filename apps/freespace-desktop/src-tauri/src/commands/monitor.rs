//! Write-monitor commands (US6, T126).
//!
//! `monitor_start` returns a task id immediately. The worker prefers the
//! `Microsoft-Windows-Kernel-File` ETW session and falls back to process I/O
//! counters when the kernel session cannot start (the usual outcome without
//! elevation). Accepted bursts are pushed into `freespace_app::monitor`; the
//! worker only converts platform events into [`MonitorSample`]s and emits
//! `monitor-tick` at 2 Hz.

#[cfg(windows)]
use std::sync::mpsc;
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use tauri::{AppHandle, Emitter, Manager, State};

#[cfg(windows)]
use freespace_app::monitor::SOURCE_ETW;
use freespace_app::{
    dto::{
        MonitorEventFilter, MonitorEventPage, MonitorSample, MonitorStartRequest, MonitorStatus,
        MonitorTick, TaskStarted, TaskState,
    },
    error::{UiError, UiErrorCode},
    monitor::{COUNTERS_UNAVAILABLE_KEY, ETW_UNAVAILABLE_KEY, MonitorStarted, SOURCE_COUNTERS},
    operations::now_millis,
};
use freespace_core::time::Timestamp;
#[cfg(not(windows))]
use freespace_platform::common::monitor::ProcessIo;
#[cfg(windows)]
use freespace_platform::{
    etw::{EtwSession, WriteEvent},
    process_io,
};
#[cfg(windows)]
use freespace_scan::monitor_fallback::FallbackMonitor;

use crate::{
    events::{EventEnvelope, MonitorTickEvent, TaskQueueEvent},
    state::AppState,
};

/// Interval between two `monitor-tick` emissions (2 Hz).
pub const TICK_INTERVAL: Duration = Duration::from_millis(500);
/// Granularity used to observe the stop flag while sleeping.
const SLEEP_SLICE: Duration = Duration::from_millis(100);

/// Starts a write-monitoring session.
#[tauri::command(rename_all = "snake_case")]
pub fn monitor_start(
    app: AppHandle,
    state: State<'_, AppState>,
    threshold: Option<u64>,
    interval: Option<u64>,
    exclusions: Option<Vec<String>>,
) -> Result<TaskStarted, UiError> {
    let request = MonitorStartRequest {
        threshold,
        interval,
        exclusions: exclusions.unwrap_or_default(),
    };
    let started =
        state
            .monitors()
            .start_with_defaults(&request, &state.monitor_defaults(), now_millis())?;
    state.register_monitor_task(&started.task_id, Arc::clone(&started.stop));

    let task_id = started.task_id.clone();
    let worker_task_id = task_id.clone();
    let handle = app.clone();
    let spawn = std::thread::Builder::new()
        .name(format!("freespace-monitor-{task_id}"))
        .spawn(move || run_worker(handle, started));

    if let Err(error) = spawn {
        let failure = UiError::new(UiErrorCode::Internal)
            .with_details(format!("cannot start the monitor worker: {error}"));
        let _ = state
            .monitors()
            .fail(&task_id, failure.clone(), now_millis());
        state.finish_task(&worker_task_id, TaskState::Failed, Some(failure));

        return Err(
            UiError::new(UiErrorCode::Internal).with_details("cannot start the monitor worker")
        );
    }

    Ok(TaskStarted { task_id })
}

/// Returns the visible state of a monitor task.
#[tauri::command(rename_all = "snake_case")]
pub fn monitor_status(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<MonitorStatus, UiError> {
    state.monitors().status(&task_id)
}

/// Stops a monitor task.
#[tauri::command(rename_all = "snake_case")]
pub fn monitor_stop(state: State<'_, AppState>, task_id: String) -> Result<(), UiError> {
    state.monitors().stop(&task_id, now_millis())?;
    state.finish_task(&task_id, TaskState::Completed, None);

    Ok(())
}

/// Returns one page of accepted monitor events, newest first.
#[tauri::command(rename_all = "snake_case")]
pub fn monitor_events_page(
    state: State<'_, AppState>,
    task_id: String,
    filter: Option<MonitorEventFilter>,
    page_size: Option<u16>,
    cursor: Option<String>,
) -> Result<MonitorEventPage, UiError> {
    state.monitors().events_page(
        &task_id,
        &filter.unwrap_or_default(),
        page_size.unwrap_or(freespace_app::monitor::DEFAULT_PAGE_SIZE),
        cursor.as_deref(),
    )
}

/// Replaces the process and path exclusions of the monitor.
#[tauri::command(rename_all = "snake_case")]
pub fn monitor_set_exclusions(
    state: State<'_, AppState>,
    exclusions: Vec<String>,
) -> Result<(), UiError> {
    state.monitors().set_exclusions(exclusions)
}

#[cfg(windows)]
fn run_worker(app: AppHandle, started: MonitorStarted) {
    let task_id = started.task_id.clone();
    let stop = started.stop;
    let (sender, receiver) = mpsc::channel::<WriteEvent>();
    let mut names: HashMap<u32, String> = HashMap::new();

    let etw = EtwSession::start(move |event| {
        let _ = sender.send(event);
    })
    .ok();

    let app_state = app.state::<AppState>();
    if etw.is_some() {
        let _ = app_state.monitors().set_source(&task_id, SOURCE_ETW);
    } else {
        let _ = app_state.monitors().set_source(&task_id, SOURCE_COUNTERS);
        let _ = app_state
            .monitors()
            .add_warning(&task_id, ETW_UNAVAILABLE_KEY);
    }

    let mut fallback = if etw.is_some() {
        None
    } else {
        let mut monitor = FallbackMonitor::new();
        let _ = monitor.poll();
        Some(monitor)
    };
    let mut counters_warned = false;
    let mut ticker = Ticker::new(TICK_INTERVAL);

    while !stop.load(Ordering::Relaxed) {
        if !sleep_until_tick(&stop, ticker.remaining()) {
            break;
        }
        ticker.reset();

        let mut samples = Vec::new();
        if etw.is_some() {
            while let Ok(event) = receiver.try_recv() {
                let process = names
                    .entry(event.process_id)
                    .or_insert_with(|| {
                        process_io::counters_for(event.process_id)
                            .map(|counters| counters.name)
                            .unwrap_or_default()
                    })
                    .clone();
                samples.push(MonitorSample {
                    timestamp: event.timestamp_ms,
                    process_id: event.process_id,
                    process,
                    bytes: event.bytes,
                    paths: vec![event.path],
                });
            }
        } else if let Some(monitor) = &mut fallback {
            match monitor.poll() {
                Ok(writes) => {
                    for write in writes {
                        samples.push(MonitorSample {
                            timestamp: Timestamp::now().as_millis(),
                            process_id: write.pid,
                            process: write.process,
                            bytes: write.bytes,
                            paths: Vec::new(),
                        });
                    }
                }
                Err(_) if !counters_warned => {
                    counters_warned = true;
                    let _ = app_state
                        .monitors()
                        .add_warning(&task_id, COUNTERS_UNAVAILABLE_KEY);
                }
                Err(_) => {}
            }
        }

        if samples.is_empty() {
            continue;
        }
        if let Ok(Some(tick)) =
            app_state
                .monitors()
                .push_samples(&task_id, &samples, Timestamp::now().as_millis())
        {
            emit_tick(&app, &app_state, &task_id, tick);
        }
    }

    if let Some(session) = etw {
        let _ = session.stop();
    }
    let _ = app_state
        .monitors()
        .stop(&task_id, Timestamp::now().as_millis());
    app_state.finish_task(&task_id, TaskState::Completed, None);
    emit_queue(&app, &app_state);
}

#[cfg(not(windows))]
fn run_worker(app: AppHandle, started: MonitorStarted) {
    let task_id = started.task_id.clone();
    let stop = started.stop;
    let app_state = app.state::<AppState>();
    let backend = freespace_platform::platform().monitor;

    // There is no ETW equivalent outside Windows; process I/O counters are the
    // only source, and the backend reports them as unavailable when unsupported.
    let _ = app_state.monitors().set_source(&task_id, SOURCE_COUNTERS);
    let _ = app_state
        .monitors()
        .add_warning(&task_id, ETW_UNAVAILABLE_KEY);

    let mut previous: HashMap<u32, ProcessIo> = HashMap::new();
    let mut counters_warned = false;
    let mut ticker = Ticker::new(TICK_INTERVAL);

    while !stop.load(Ordering::Relaxed) {
        if !sleep_until_tick(&stop, ticker.remaining()) {
            break;
        }
        ticker.reset();

        let snapshot = match backend.snapshot() {
            Ok(snapshot) => snapshot,
            Err(_) => {
                if !counters_warned {
                    counters_warned = true;
                    let _ = app_state
                        .monitors()
                        .add_warning(&task_id, COUNTERS_UNAVAILABLE_KEY);
                }
                continue;
            }
        };

        let now = Timestamp::now().as_millis();
        let samples: Vec<MonitorSample> = snapshot
            .iter()
            .filter_map(|current| {
                let before = previous.get(&current.pid)?;
                let bytes = current.write_bytes.checked_sub(before.write_bytes)?;
                if bytes == 0 {
                    return None;
                }

                Some(MonitorSample {
                    timestamp: now,
                    process_id: current.pid,
                    process: current.name.clone(),
                    bytes,
                    paths: Vec::new(),
                })
            })
            .collect();
        previous = snapshot
            .into_iter()
            .map(|process| (process.pid, process))
            .collect();

        if samples.is_empty() {
            continue;
        }
        if let Ok(Some(tick)) =
            app_state
                .monitors()
                .push_samples(&task_id, &samples, Timestamp::now().as_millis())
        {
            emit_tick(&app, &app_state, &task_id, tick);
        }
    }

    let _ = app_state
        .monitors()
        .stop(&task_id, Timestamp::now().as_millis());
    app_state.finish_task(&task_id, TaskState::Completed, None);
    emit_queue(&app, &app_state);
}

fn emit_tick(app: &AppHandle, state: &AppState, task_id: &str, tick: MonitorTick) {
    let envelope = EventEnvelope::new(
        "monitor-tick",
        state.next_event_sequence(),
        Some(task_id.to_owned()),
        MonitorTickEvent {
            task_id: task_id.to_owned(),
            sequence: tick.sequence,
            events: tick.events,
        },
    );
    let _ = app.emit("monitor-tick", envelope);
}

fn emit_queue(app: &AppHandle, state: &AppState) {
    let envelope = EventEnvelope::new(
        "task-queue-changed",
        state.next_event_sequence(),
        None,
        TaskQueueEvent {
            tasks: state.tasks(),
        },
    );
    let _ = app.emit("task-queue-changed", envelope);
}

/// Waits for the next tick while observing the stop flag.
fn sleep_until_tick(stop: &Arc<AtomicBool>, duration: Duration) -> bool {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        if stop.load(Ordering::Relaxed) {
            return false;
        }
        std::thread::sleep(SLEEP_SLICE.min(deadline.saturating_duration_since(Instant::now())));
    }

    !stop.load(Ordering::Relaxed)
}

#[derive(Debug)]
struct Ticker {
    interval: Duration,
    started: Instant,
}

impl Ticker {
    fn new(interval: Duration) -> Self {
        Self {
            interval,
            started: Instant::now(),
        }
    }

    fn remaining(&self) -> Duration {
        self.interval
            .saturating_sub(self.started.elapsed())
            .max(Duration::from_millis(1))
    }

    fn reset(&mut self) {
        self.started = Instant::now();
    }
}

/// Returns `true` when the event name is part of the monitor contract; used by
/// the capability test.
#[must_use]
pub fn is_monitor_event(name: &str) -> bool {
    name == "monitor-tick"
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    };

    use super::{TICK_INTERVAL, is_monitor_event, sleep_until_tick};
    use crate::events::EVENT_NAMES;

    #[test]
    fn ticks_twice_per_second() {
        assert!(TICK_INTERVAL <= Duration::from_millis(1_000));
        assert!(TICK_INTERVAL >= Duration::from_millis(500));
        assert!(is_monitor_event("monitor-tick"));
        assert!(EVENT_NAMES.contains(&"monitor-tick"));
    }

    #[test]
    fn the_sleep_observes_the_stop_flag() {
        let stop = Arc::new(AtomicBool::new(true));
        assert!(!sleep_until_tick(&stop, Duration::from_millis(200)));

        let stop = Arc::new(AtomicBool::new(false));
        assert!(sleep_until_tick(&stop, Duration::from_millis(10)));
        assert!(!stop.load(Ordering::Relaxed));
    }

    #[test]
    fn the_registry_exposes_the_monitor_service() {
        let directory = tempfile::tempdir().expect("temp dir");
        let state = crate::state::AppState::new(directory.path()).expect("state");
        assert!(state.monitors().active_task_id().is_none());
        assert_eq!(
            freespace_app::monitor::MonitorService::new()
                .exclusions()
                .len(),
            0
        );
    }
}
