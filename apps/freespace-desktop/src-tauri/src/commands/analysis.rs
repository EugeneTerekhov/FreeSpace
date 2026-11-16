//! Analysis commands: top files, types, duplicates, cleanup and snapshots.
//!
//! Commands only resolve the current generation and call `freespace-app`
//! services. Duplicate searches run on a worker thread and report progress via
//! events; snapshots are created and compared synchronously because the payload
//! is already in memory.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use freespace_app::{
    cleanup,
    dto::{
        DEFAULT_PAGE_SIZE, DuplicateGroupPage, DuplicateGroupView, DuplicateStartRequest,
        DuplicateTask, FilterSet, Metric, SnapshotCreateRequest, SnapshotCreated, SnapshotDiffPage,
        SnapshotDiffSort, SnapshotSummary, SortSpec, Task, TaskKind, TaskStarted, TaskState,
        TreePage, TypeSummary,
    },
    duplicates::{self},
    error::UiError,
    snapshots::SnapshotService,
    top_types,
};
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

use crate::{
    events::{
        DuplicateCompletedEvent, DuplicateProgressEvent, EventEnvelope, ProgressCounts,
        TaskQueueEvent,
    },
    state::AppState,
};

/// One page of the largest files.
#[tauri::command(rename_all = "snake_case")]
#[allow(clippy::too_many_arguments)]
pub fn top_files_page(
    state: State<'_, AppState>,
    generation: u64,
    limit: Option<usize>,
    metric: Option<Metric>,
    sort: Option<SortSpec>,
    filter: Option<FilterSet>,
    page_size: Option<u16>,
    cursor: Option<String>,
) -> Result<TreePage, UiError> {
    let snapshot = state.snapshot_for(generation)?;
    top_types::top_files_page(
        &snapshot,
        limit.unwrap_or(100),
        metric.unwrap_or_default(),
        sort.unwrap_or_default(),
        &filter.unwrap_or_default(),
        page_size.unwrap_or(DEFAULT_PAGE_SIZE),
        cursor.as_deref(),
    )
}

/// Aggregates the current generation by extension.
#[tauri::command(rename_all = "snake_case")]
pub fn types_summary(
    state: State<'_, AppState>,
    generation: u64,
    filter: Option<FilterSet>,
    group_by: Option<String>,
) -> Result<Vec<TypeSummary>, UiError> {
    let snapshot = state.snapshot_for(generation)?;
    let filter = filter.unwrap_or_default();
    if group_by.as_deref() == Some("category") {
        top_types::types_category_summary(&snapshot, &filter)
    } else {
        top_types::types_summary(&snapshot, &filter)
    }
}

/// One page of files of a selected extension or category.
#[tauri::command(rename_all = "snake_case")]
#[allow(clippy::too_many_arguments)]
pub fn types_files_page(
    state: State<'_, AppState>,
    generation: u64,
    file_type: String,
    metric: Option<Metric>,
    sort: Option<SortSpec>,
    filter: Option<FilterSet>,
    page_size: Option<u16>,
    cursor: Option<String>,
) -> Result<TreePage, UiError> {
    let snapshot = state.snapshot_for(generation)?;
    top_types::types_files_page(
        &snapshot,
        &file_type,
        metric.unwrap_or_default(),
        sort.unwrap_or_default(),
        &filter.unwrap_or_default(),
        page_size.unwrap_or(DEFAULT_PAGE_SIZE),
        cursor.as_deref(),
    )
}

/// Starts a duplicate search on a worker thread.
#[tauri::command(rename_all = "snake_case")]
pub fn duplicates_start(
    app: AppHandle,
    state: State<'_, AppState>,
    request: DuplicateStartRequest,
) -> Result<TaskStarted, UiError> {
    let snapshot = state.snapshot()?;
    let requested = crate::security::canonicalize(std::path::Path::new(&request.path))?;
    let root = snapshot
        .root_id()
        .map(|id| snapshot.node_row(id).map(|row| row.path))
        .transpose()?
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(snapshot.volume().mount_point.as_os_str()));
    if !crate::security::is_within(&root, &requested)
        && !crate::security::is_within(&requested, &root)
    {
        return Err(UiError::validation(
            "start a duplicate search inside the scanned area",
        ));
    }
    if request.exclusions.len() > freespace_app::validation::MAX_EXCLUSIONS {
        return Err(UiError::validation("too many exclusions"));
    }

    let task_id = Uuid::new_v4().to_string();
    let cancel = Arc::new(AtomicBool::new(false));
    state.reserve_duplicates(
        &task_id,
        Some(snapshot.volume().id.to_string()),
        cancel.clone(),
    )?;

    let handle = app.clone();
    let worker_task = task_id.clone();
    let algorithm = request.algorithm;
    let keep_rule = request.keep_rule;
    let min_size = request.min_size;
    let exclusions = request.exclusions;
    std::thread::spawn(move || {
        let app_state = handle.state::<AppState>();
        emit_progress(&handle, &app_state, &worker_task, 0);
        let result = duplicates::search_with_exclusions(
            &snapshot,
            min_size,
            &algorithm,
            &keep_rule,
            &exclusions,
        );
        let (state, error, groups, reclaimable) = match result {
            Ok(search) => {
                let cancelled = cancel.load(Ordering::Relaxed);
                let state = if cancelled {
                    TaskState::Cancelled
                } else {
                    let state = duplicates::task_state(&search, TaskState::Completed);
                    let groups = search.groups.len() as u64;
                    let reclaimable = search.reclaimable_bytes;
                    app_state.install_duplicates(&worker_task, Arc::new(search));
                    return finish(
                        &handle,
                        &app_state,
                        &worker_task,
                        state,
                        groups,
                        reclaimable,
                        None,
                    );
                };
                (state, None, 0, 0)
            }
            Err(error) => (TaskState::Failed, Some(error), 0, 0),
        };
        finish(
            &handle,
            &app_state,
            &worker_task,
            state,
            groups,
            reclaimable,
            error,
        );
    });

    Ok(TaskStarted { task_id })
}

fn emit_progress(app: &AppHandle, state: &AppState, task_id: &str, processed: u64) {
    let envelope = EventEnvelope::new(
        "duplicates-progress",
        state.next_event_sequence(),
        Some(task_id.to_owned()),
        DuplicateProgressEvent {
            progress: ProgressCounts {
                processed,
                total: None,
            },
            phase_key: "duplicates.phase.hashing".to_owned(),
        },
    );
    let _ = app.emit("duplicates-progress", envelope);
    let queue = EventEnvelope::new(
        "task-queue-changed",
        state.next_event_sequence(),
        None,
        TaskQueueEvent {
            tasks: state.tasks(),
        },
    );
    let _ = app.emit("task-queue-changed", queue);
}

fn finish(
    app: &AppHandle,
    state: &AppState,
    task_id: &str,
    task_state: TaskState,
    groups: u64,
    reclaimable: u64,
    error: Option<UiError>,
) {
    state.finish_task(task_id, task_state, error.clone());
    state.release_duplicates();
    let envelope = EventEnvelope::new(
        "duplicates-completed",
        state.next_event_sequence(),
        Some(task_id.to_owned()),
        DuplicateCompletedEvent {
            task_id: task_id.to_owned(),
            state: task_state,
            groups_count: groups,
            reclaimable_bytes: reclaimable,
            error,
        },
    );
    let _ = app.emit("duplicates-completed", envelope);
    let queue = EventEnvelope::new(
        "task-queue-changed",
        state.next_event_sequence(),
        None,
        TaskQueueEvent {
            tasks: state.tasks(),
        },
    );
    let _ = app.emit("task-queue-changed", queue);
}

/// Returns the visible state and counters of a duplicate search.
#[tauri::command(rename_all = "snake_case")]
pub fn duplicates_status(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<DuplicateTask, UiError> {
    let task = state.task(&task_id)?;
    let search = state.duplicates(&task_id).ok();
    let (groups_count, candidates, hashed, reclaimable, warnings) =
        search.as_ref().map_or((0, 0, 0, 0, Vec::new()), |search| {
            (
                search.groups.len() as u64,
                search.candidates,
                search.hashed,
                search.reclaimable_bytes,
                search.warnings.clone(),
            )
        });

    Ok(DuplicateTask {
        task_id,
        state: task.state,
        groups_count,
        candidates,
        hashed,
        reclaimable_bytes: reclaimable,
        warnings,
        error: task.error,
    })
}

/// Returns one page of groups of a finished search.
#[tauri::command(rename_all = "snake_case")]
pub fn duplicates_groups_page(
    state: State<'_, AppState>,
    task_id: String,
    cursor: Option<String>,
    page_size: Option<u16>,
) -> Result<DuplicateGroupPage, UiError> {
    let search = state.duplicates(&task_id)?;
    duplicates::groups_page(
        &task_id,
        &search,
        page_size.unwrap_or(DEFAULT_PAGE_SIZE),
        cursor.as_deref(),
    )
}

/// Returns one group of a finished search.
#[tauri::command(rename_all = "snake_case")]
pub fn duplicates_group(
    state: State<'_, AppState>,
    task_id: String,
    group_id: String,
) -> Result<DuplicateGroupView, UiError> {
    let search = state.duplicates(&task_id)?;
    duplicates::group(&search, &group_id)
}

/// Requests cancellation of a duplicate search.
#[tauri::command(rename_all = "snake_case")]
pub fn duplicates_cancel(state: State<'_, AppState>, task_id: String) -> Result<(), UiError> {
    state.cancel_task(&task_id)
}

/// Returns cleanup recommendations for the current generation.
#[tauri::command(rename_all = "snake_case")]
pub fn cleanup_candidates(
    state: State<'_, AppState>,
    generation: u64,
    older_than_days: Option<u32>,
    filter: Option<FilterSet>,
) -> Result<Vec<freespace_app::dto::CleanupCandidateView>, UiError> {
    let snapshot = state.snapshot_for(generation)?;
    cleanup::cleanup_candidates(
        &snapshot,
        older_than_days.unwrap_or(180),
        &filter.unwrap_or_default(),
    )
}

/// Returns one page of directories without files.
#[tauri::command(rename_all = "snake_case")]
pub fn cleanup_empty_dirs_page(
    state: State<'_, AppState>,
    generation: u64,
    metric: Option<Metric>,
    sort: Option<SortSpec>,
    cursor: Option<String>,
    page_size: Option<u16>,
) -> Result<TreePage, UiError> {
    let snapshot = state.snapshot_for(generation)?;
    cleanup::empty_dirs_page(
        &snapshot,
        metric.unwrap_or_default(),
        sort.unwrap_or_default(),
        page_size.unwrap_or(DEFAULT_PAGE_SIZE),
        cursor.as_deref(),
    )
}

/// Lists stored snapshots.
#[tauri::command(rename_all = "snake_case")]
pub fn snapshots_list(
    state: State<'_, AppState>,
    volume_id: Option<String>,
) -> Result<Vec<SnapshotSummary>, UiError> {
    state.snapshots().list(volume_id.as_deref())
}

/// Creates a snapshot from the current generation.
#[tauri::command(rename_all = "snake_case")]
pub fn snapshots_create(
    state: State<'_, AppState>,
    request: SnapshotCreateRequest,
) -> Result<SnapshotCreated, UiError> {
    let snapshot = state.snapshot()?;
    state.snapshots().create(&snapshot, &request)
}

/// Compares two stored snapshots.
#[tauri::command(rename_all = "snake_case")]
pub fn snapshots_compare(
    state: State<'_, AppState>,
    base_id: String,
    target_id: String,
) -> Result<TaskStarted, UiError> {
    let task_id = state.snapshots().compare(&base_id, &target_id)?;

    Ok(TaskStarted { task_id })
}

/// Returns one page of a cached comparison.
#[tauri::command(rename_all = "snake_case")]
pub fn snapshots_diff_page(
    state: State<'_, AppState>,
    task_id: String,
    cursor: Option<String>,
    page_size: Option<u16>,
    sort: Option<SnapshotDiffSort>,
) -> Result<SnapshotDiffPage, UiError> {
    state.snapshots().diff_page(
        &task_id,
        sort.unwrap_or_default(),
        page_size.unwrap_or(DEFAULT_PAGE_SIZE),
        cursor.as_deref(),
    )
}

/// Deletes a stored snapshot.
#[tauri::command(rename_all = "snake_case")]
pub fn snapshots_delete(state: State<'_, AppState>, snapshot_id: String) -> Result<(), UiError> {
    state.snapshots().delete(&snapshot_id)
}

/// Returns the state of a comparison; comparisons are synchronous.
#[tauri::command(rename_all = "snake_case")]
pub fn snapshots_status(state: State<'_, AppState>, task_id: String) -> Result<Task, UiError> {
    let _ = state;
    Ok(Task {
        task_id,
        kind: TaskKind::SnapshotCompare,
        state: TaskState::Completed,
        volume_id: None,
        progress: None,
        started_at: freespace_core::time::Timestamp::now().as_millis(),
        finished_at: Some(freespace_core::time::Timestamp::now().as_millis()),
        cancelable: false,
        error: None,
    })
}

/// Default snapshot directory for diagnostics.
#[must_use]
pub fn snapshot_directory(state: &AppState) -> std::path::PathBuf {
    SnapshotService::default_dir(&state.data_dir())
}
