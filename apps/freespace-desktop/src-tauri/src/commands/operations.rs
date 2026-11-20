//! Destructive-operation and journal commands.
//!
//! Every command goes through `freespace-app::operations`: a preview token is
//! required, the volume is locked during execution and the result is written to
//! the journal by `freespace-ops`. Commands never implement deletion themselves.

use freespace_app::{
    dto::{
        DeleteMode, JournalEntryView, JournalPage, OperationAccepted, OperationKind,
        OperationOutcome, OperationPreview, OperationPreviewRequest, OperationProgress,
    },
    error::{UiError, UiErrorCode},
    operations::{self, OperationService, now_millis},
};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{
    events::{EventEnvelope, OperationCompletedEvent},
    state::AppState,
};

/// Builds a preview and stores its one-time token.
#[tauri::command(rename_all = "snake_case")]
pub fn operation_preview(
    state: State<'_, AppState>,
    request: OperationPreviewRequest,
) -> Result<OperationPreview, UiError> {
    let mut request = request;
    if request.kind == OperationKind::Delete && request.task_id.is_some() {
        let task_id = request.task_id.clone().unwrap_or_default();
        let guarded = state.duplicate_targets(&task_id, &request.targets)?;
        request.targets = guarded
            .into_iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
    }
    let snapshot = state.snapshot()?;
    let roots = snapshot.root_paths();
    for target in &request.targets {
        let target = std::path::Path::new(target);
        if !roots
            .iter()
            .any(|root| crate::security::is_within(root, target))
        {
            return Err(UiError::new(UiErrorCode::AccessDenied)
                .with_details("the target is outside the scanned area"));
        }
    }

    state.operations().preview(&request, None, now_millis())
}

/// Confirms a preview and starts the operation on a worker thread.
#[tauri::command(rename_all = "snake_case")]
pub fn operation_confirm(
    app: AppHandle,
    state: State<'_, AppState>,
    preview_id: String,
    confirmation_token: String,
    delete_mode: Option<DeleteMode>,
    extra_confirmation: Option<bool>,
) -> Result<OperationAccepted, UiError> {
    let elevated = freespace_platform::platform().elevation.is_elevated();
    let prepared = state.operations().take(
        &preview_id,
        &confirmation_token,
        delete_mode,
        extra_confirmation.unwrap_or(false),
        elevated,
        now_millis(),
    )?;
    let operation_id = prepared.operation_id.to_string();
    let volume_id = state
        .snapshot()
        .ok()
        .and_then(|snapshot| {
            prepared
                .targets
                .iter()
                .find_map(|target| snapshot.volume_for_path(target))
                .or_else(|| snapshot.volumes().first())
                .map(|volume| volume.id.to_string())
        })
        .unwrap_or_default();
    state
        .operations()
        .begin(&operation_id, &volume_id, prepared.targets.len() as u64)?;
    let handle = app.clone();
    let worker_id = operation_id.clone();

    std::thread::spawn(move || {
        let app_state = handle.state::<AppState>();
        let result = app_state.with_store(|store| {
            let mut on_progress = |progress: &OperationProgress| {
                emit_progress(&handle, &app_state, progress.clone());
            };
            operations::execute_delete(
                app_state.operations(),
                store,
                &prepared,
                &volume_id,
                None,
                &mut on_progress,
            )
        });
        match result {
            Ok(outcome) => emit_completed(&handle, &app_state, &outcome, None),
            Err(error) => emit_failure(&handle, &app_state, &worker_id, error),
        }
    });

    Ok(OperationAccepted { operation_id })
}

/// Returns the current state of an operation.
#[tauri::command(rename_all = "snake_case")]
pub fn operation_status(
    state: State<'_, AppState>,
    operation_id: String,
) -> Result<OperationProgress, UiError> {
    state.operations().status(&operation_id)
}

/// Requests cooperative cancellation.
#[tauri::command(rename_all = "snake_case")]
pub fn operation_cancel(state: State<'_, AppState>, operation_id: String) -> Result<(), UiError> {
    state.operations().cancel(&operation_id)
}

/// Reverses the renames of a finished operation.
#[tauri::command(rename_all = "snake_case")]
pub fn operation_undo(
    state: State<'_, AppState>,
    operation_id: String,
) -> Result<OperationAccepted, UiError> {
    state.with_store(|store| operations::undo(store, &operation_id))?;

    Ok(OperationAccepted { operation_id })
}

/// Returns one page of the journal.
#[tauri::command(rename_all = "snake_case")]
pub fn journal_page(
    state: State<'_, AppState>,
    filter: Option<freespace_app::dto::JournalFilter>,
    page_size: Option<u16>,
    cursor: Option<String>,
) -> Result<JournalPage, UiError> {
    state.with_store(|store| {
        operations::journal_page(
            store,
            &filter.unwrap_or_default(),
            page_size.unwrap_or(500),
            cursor.as_deref(),
        )
    })
}

/// Returns one journal row.
#[tauri::command(rename_all = "snake_case")]
pub fn journal_entry(
    state: State<'_, AppState>,
    entry_id: String,
) -> Result<JournalEntryView, UiError> {
    state.with_store(|store| operations::journal_entry(store, &entry_id))
}

/// Reverses the renames of a journal entry.
#[tauri::command(rename_all = "snake_case")]
pub fn journal_undo(
    state: State<'_, AppState>,
    entry_id: String,
) -> Result<OperationAccepted, UiError> {
    state.with_store(|store| {
        let entry = operations::journal_entry(store, &entry_id)?;
        operations::undo(store, &entry.operation_id)?;

        Ok(OperationAccepted {
            operation_id: entry.operation_id,
        })
    })
}

/// Returns the operation service for tests.
#[must_use]
pub const fn service(state: &AppState) -> &OperationService {
    state.operations()
}

fn emit_progress(app: &AppHandle, state: &AppState, progress: OperationProgress) {
    let envelope = EventEnvelope::new(
        "operation-progress",
        state.next_event_sequence(),
        None,
        progress,
    );
    let _ = app.emit("operation-progress", envelope);
}

fn emit_completed(
    app: &AppHandle,
    state: &AppState,
    outcome: &OperationOutcome,
    error: Option<UiError>,
) {
    let envelope = EventEnvelope::new(
        "operation-completed",
        state.next_event_sequence(),
        None,
        OperationCompletedEvent {
            operation_id: outcome.operation_id.clone(),
            state: outcome.state,
            journal_entry_id: outcome.journal_entry_id.clone(),
            undo_available: outcome.undo_available,
            error,
        },
    );
    let _ = app.emit("operation-completed", envelope);
}

fn emit_failure(app: &AppHandle, state: &AppState, operation_id: &str, error: UiError) {
    let envelope = EventEnvelope::new(
        "operation-completed",
        state.next_event_sequence(),
        None,
        OperationCompletedEvent {
            operation_id: operation_id.to_owned(),
            state: freespace_app::dto::OperationState::Failed,
            journal_entry_id: None,
            undo_available: false,
            error: Some(error),
        },
    );
    let _ = app.emit("operation-completed", envelope);
}
