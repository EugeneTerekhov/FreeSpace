//! Shredder commands (US6, T125).
//!
//! `shred_preview` builds a normal operation preview with the extra SSD and
//! irreversibility warnings; `shred_start` consumes its one-time token and runs
//! the overwrite on a worker thread. All file-system work stays in
//! `freespace-ops::shred`.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Emitter, Manager, State};

use freespace_app::{
    dto::{
        OperationAccepted, OperationOutcome, OperationPreview, OperationProgress,
        ShredPatternChoice, ShredPreviewRequest,
    },
    error::{UiError, UiErrorCode},
    operations::now_millis,
    shred,
};

use crate::{
    events::{EventEnvelope, OperationCompletedEvent},
    security,
    state::AppState,
};

/// Builds a shred preview; the result carries one-time confirmation credentials.
#[tauri::command(rename_all = "snake_case")]
pub fn shred_preview(
    state: State<'_, AppState>,
    targets: Vec<String>,
    passes: u32,
    pattern: ShredPatternChoice,
) -> Result<OperationPreview, UiError> {
    ensure_targets(&state, &targets)?;
    let request = ShredPreviewRequest {
        targets,
        passes,
        pattern,
    };

    state
        .shredder()
        .preview(state.operations(), &request, None, now_millis())
}

/// Confirms a shred preview and starts the worker.
///
/// Calling this command is the dedicated irreversible confirmation: the client
/// only invokes it from the destructive button of the shredder dialog.
#[tauri::command(rename_all = "snake_case")]
pub fn shred_start(
    app: AppHandle,
    state: State<'_, AppState>,
    preview_id: String,
    confirmation_token: String,
    extra_confirmation: Option<bool>,
) -> Result<OperationAccepted, UiError> {
    let elevated = freespace_platform::elevation::is_elevated();
    let (prepared, plan) = state.shredder().take(
        state.operations(),
        &preview_id,
        &confirmation_token,
        elevated,
        now_millis(),
    )?;
    // The command may only be reached through the dedicated confirmation. The
    // explicit flag still allows a stricter client to opt out.
    if extra_confirmation == Some(false) {
        return Err(UiError::validation(
            "shredding needs the dedicated irreversible confirmation",
        ));
    }

    let operation_id = prepared.operation_id.to_string();
    let volume_id = state
        .snapshot()
        .map(|snapshot| snapshot.volume().id.to_string())
        .unwrap_or_default();
    let handle = app.clone();
    let worker_id = operation_id.clone();

    std::thread::Builder::new()
        .name(format!("freespace-shred-{operation_id}"))
        .spawn(move || {
            let app_state = handle.state::<AppState>();
            let result = app_state.with_store(|store| {
                let mut on_progress = |progress: &OperationProgress| {
                    emit_progress(&handle, &app_state, progress.clone());
                };
                shred::execute(
                    app_state.operations(),
                    store,
                    &prepared,
                    plan,
                    &volume_id,
                    &mut on_progress,
                )
            });
            match result {
                Ok(outcome) => emit_completed(&handle, &app_state, &outcome, None),
                Err(error) => emit_failure(&handle, &app_state, &worker_id, error),
            }
        })
        .map_err(|error| {
            UiError::new(UiErrorCode::Internal)
                .with_details(format!("cannot start the shred worker: {error}"))
        })?;

    Ok(OperationAccepted { operation_id })
}

/// Returns the progress of a shred run.
#[tauri::command(rename_all = "snake_case")]
pub fn shred_status(
    state: State<'_, AppState>,
    operation_id: String,
) -> Result<OperationProgress, UiError> {
    state.operations().status(&operation_id)
}

/// Requests cooperative cancellation between two targets.
#[tauri::command(rename_all = "snake_case")]
pub fn shred_cancel(state: State<'_, AppState>, operation_id: String) -> Result<(), UiError> {
    state.operations().cancel(&operation_id)
}

fn ensure_targets(state: &AppState, targets: &[String]) -> Result<(), UiError> {
    if targets.is_empty() {
        return Err(UiError::validation("a shred run needs at least one target"));
    }

    let root = state.snapshot().ok().map(|snapshot| {
        snapshot
            .root_id()
            .and_then(|root| snapshot.node_row(root).ok())
            .map(|row| PathBuf::from(row.path))
            .unwrap_or_else(|| PathBuf::from(snapshot.volume().mount_point.as_os_str()))
    });

    for target in targets {
        let path = freespace_app::validation::validate_path(target)?;
        if let Some(root) = &root {
            if !within(root, &path) {
                return Err(UiError::new(UiErrorCode::AccessDenied)
                    .with_details(format!("`{target}` is outside the scanned area")));
            }
        }
    }

    Ok(())
}

fn within(root: &Path, target: &Path) -> bool {
    security::is_within(root, target)
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
            undo_available: false,
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::within;

    #[test]
    fn scope_comparison_is_case_insensitive() {
        assert!(within(
            Path::new(r"C:\Users\Public"),
            Path::new(r"c:\users\public\file.bin")
        ));
        assert!(!within(
            Path::new(r"C:\Users\Public"),
            Path::new(r"C:\Users\Publicity\file.bin")
        ));
    }

    #[test]
    fn shred_service_is_available_on_the_state() {
        let directory = tempfile::tempdir().expect("temp dir");
        let state = crate::state::AppState::new(directory.path()).expect("state");
        assert_eq!(state.shredder().plan_count(), 0);
    }
}
