//! Scan commands and the background worker.
//!
//! `scan_start` returns immediately with a task id; the worker runs the blocking
//! scanner on a dedicated thread, throttles progress to 20 Hz and emits
//! `scan-progress` / `scan-completed`. One scan per volume is enforced by the
//! task registry.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use tauri::{AppHandle, Emitter, Manager, State};

use freespace_app::{
    dto::{ScanRequest, ScanSessionSummary, ScanStarted, Task, TaskState},
    error::UiError,
    scan::run_scan,
    validation::{validate_path, validate_scan_request},
};
use freespace_core::model::ScanIssue;
use freespace_core::model::ScanSession;
use freespace_scan::session::{ScanObserver, ScanPhase, ScanProgress};

use crate::{
    events::{
        EventEnvelope, EventThrottle, ScanCompletedEvent, ScanSummary, Sequence, TaskQueueEvent,
        scan_progress_event, task_progress,
    },
    security,
    state::AppState,
};

/// Starts a background scan.
#[tauri::command(rename_all = "snake_case")]
pub fn scan_start(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ScanRequest,
) -> Result<ScanStarted, UiError> {
    validate_scan_request(&request)?;
    let path = validate_path(&request.path)?;
    security::canonicalize(&path)?;

    let volume_id = freespace_platform::volumes::describe_volume(&path)?
        .into_volume()
        .id
        .to_string();
    let generation = state.next_generation()?;
    let task_id = uuid::Uuid::new_v4().to_string();
    let cancel = Arc::new(AtomicBool::new(false));
    state.reserve_scan(&task_id, &volume_id, Arc::clone(&cancel))?;

    let worker_task_id = task_id.clone();
    let worker_volume_id = volume_id.clone();
    let worker_app = app.clone();
    std::thread::Builder::new()
        .name(format!("freespace-scan-{task_id}"))
        .spawn(move || {
            run_worker(
                worker_app,
                worker_task_id,
                worker_volume_id,
                generation,
                request,
                cancel,
            );
        })
        .map_err(|error| {
            state.release_scan(&volume_id);
            UiError::new(freespace_app::error::UiErrorCode::Internal)
                .with_details(error.to_string())
        })?;

    Ok(ScanStarted {
        task_id,
        generation,
    })
}

/// Returns one task.
#[tauri::command(rename_all = "snake_case")]
pub fn scan_status(state: State<'_, AppState>, task_id: String) -> Result<Task, UiError> {
    state.task(&task_id)
}

/// Requests cancellation of a task.
#[tauri::command(rename_all = "snake_case")]
pub fn scan_cancel(state: State<'_, AppState>, task_id: String) -> Result<(), UiError> {
    state.cancel_task(&task_id)
}

/// Returns finished scan sessions, newest first.
#[tauri::command(rename_all = "snake_case")]
#[must_use]
pub fn scan_sessions(state: State<'_, AppState>) -> Vec<ScanSessionSummary> {
    state.scan_sessions()
}

struct ScanWorkerObserver {
    app: AppHandle,
    task_id: String,
    cancel: Arc<AtomicBool>,
    throttle: EventThrottle,
    sequence: Sequence,
    started: Instant,
    last_emit: Instant,
    last_bytes: u64,
}

impl ScanObserver for ScanWorkerObserver {
    fn progress(&mut self, progress: &ScanProgress) {
        let now = Instant::now();
        if progress.phase != ScanPhase::Finished && !self.throttle.allow(now) {
            return;
        }

        let state = self.app.state::<AppState>();
        state.update_task_progress(&self.task_id, task_progress(progress, self.started));

        let (payload, bytes) = scan_progress_event(
            progress,
            now.saturating_duration_since(self.last_emit),
            self.last_bytes,
        );
        self.last_bytes = bytes;
        self.last_emit = now;

        let envelope = EventEnvelope::new(
            "scan-progress",
            self.sequence.advance(),
            Some(self.task_id.clone()),
            payload,
        );
        let _ = self.app.emit("scan-progress", envelope);
    }

    fn issue(&mut self, _issue: &ScanIssue) {}

    fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

#[allow(clippy::too_many_arguments)]
fn run_worker(
    app: AppHandle,
    task_id: String,
    volume_id: String,
    generation: u64,
    request: ScanRequest,
    cancel: Arc<AtomicBool>,
) {
    let started = Instant::now();
    let mut observer = ScanWorkerObserver {
        app: app.clone(),
        task_id: task_id.clone(),
        cancel,
        throttle: EventThrottle::new(),
        sequence: Sequence::default(),
        started,
        last_emit: started,
        last_bytes: 0,
    };

    let result = run_scan(&request, &mut observer);
    let state = app.state::<AppState>();
    let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

    match result {
        Ok(outcome) => {
            let freespace_app::scan::ScanOutcome {
                tree,
                session,
                method,
                volume,
                warnings,
                issues,
                cancelled,
                ..
            } = outcome;

            let task_state = if cancelled {
                TaskState::Cancelled
            } else if issues.is_empty() {
                TaskState::Completed
            } else {
                TaskState::Partial
            };
            let summary = ScanSummary {
                nodes_count: session.nodes_count,
                method: method.as_str().to_owned(),
                roots: session
                    .roots
                    .iter()
                    .map(|root| freespace_platform::paths::display(root))
                    .collect(),
                warnings,
                issues_count: u64::try_from(issues.len()).unwrap_or(u64::MAX),
                elapsed_ms,
            };
            let session_summary = session_summary(&session);

            state.install_snapshot(
                Arc::new(freespace_app::tree::TreeSnapshot::new(
                    tree,
                    generation,
                    volume,
                    Some(session),
                )),
                Some(session_summary),
            );
            state.finish_task(&task_id, task_state, None);

            let event = ScanCompletedEvent {
                task_id: task_id.clone(),
                state: task_state,
                generation,
                summary: Some(summary),
                error: None,
            };
            emit_completed(&app, &mut observer, event);
        }
        Err(error) => {
            state.finish_task(&task_id, TaskState::Failed, Some(error.clone()));
            let event = ScanCompletedEvent {
                task_id: task_id.clone(),
                state: TaskState::Failed,
                generation,
                summary: None,
                error: Some(error),
            };
            emit_completed(&app, &mut observer, event);
        }
    }

    state.release_scan(&volume_id);
    let tasks = TaskQueueEvent {
        tasks: app.state::<AppState>().tasks(),
    };
    let _ = app.emit(
        "task-queue-changed",
        EventEnvelope::new(
            "task-queue-changed",
            observer.sequence.advance(),
            None,
            tasks,
        ),
    );
}

fn emit_completed(app: &AppHandle, observer: &mut ScanWorkerObserver, event: ScanCompletedEvent) {
    let _ = app.emit(
        "scan-completed",
        EventEnvelope::new(
            "scan-completed",
            observer.sequence.advance(),
            Some(event.task_id.clone()),
            event,
        ),
    );
}

fn session_summary(session: &ScanSession) -> ScanSessionSummary {
    ScanSessionSummary {
        session_id: session.id.to_string(),
        volume_id: session.volume_id.to_string(),
        method: session.method.as_str().to_owned(),
        started_at: session.started_at.as_millis(),
        finished_at: session
            .finished_at
            .map(freespace_core::time::Timestamp::as_millis),
        status: session.status.as_str().to_owned(),
        nodes_count: session.nodes_count,
        roots: session
            .roots
            .iter()
            .map(|root| freespace_platform::paths::display(root))
            .collect(),
        issues_count: u64::try_from(session.issues.len()).unwrap_or(u64::MAX),
    }
}
