//! Inspection commands: metadata, owners, archives, reports and unlocking.
//!
//! Every command validates its input, resolves the generation through
//! [`AppState`] and calls a `freespace-app` service. Archive listing never
//! extracts payloads; exports run on a worker thread and are polled through
//! `export_status`, so no new event types are needed.

use std::path::Path;

use freespace_app::{
    archives,
    dto::{
        ArchiveEntryPage, DEFAULT_PAGE_SIZE, ExportComposition, ExportFormat, ExportPreview,
        ExportPreviewRequest, ExportResult, ExportScope, ExportStatus, FileLock, FilterSet,
        MetadataView, Metric, OperationAccepted, OwnerSummaryPage, SortSpec, TaskStarted, TreePage,
        UnlockMode,
    },
    error::{UiError, UiErrorCode},
    export, metadata, owners, unlock,
};
use tauri::State;

use crate::state::AppState;

/// Projects metadata of one node.
///
/// # Errors
/// Returns `E_STALE_NODE`, `E_NOT_FOUND`, `E_ACCESS_DENIED` or `E_UNSUPPORTED`.
#[tauri::command(rename_all = "snake_case")]
pub fn metadata_get(
    state: State<'_, AppState>,
    generation: u64,
    node_id: u32,
) -> Result<MetadataView, UiError> {
    let snapshot = state.snapshot_for(generation)?;
    metadata::metadata_view(&snapshot, node_id)
}

/// Returns one page of owner summaries.
///
/// # Errors
/// Returns `E_STALE_CURSOR` for a foreign cursor and `E_IO` when ownership
/// cannot be read.
#[tauri::command(rename_all = "snake_case")]
pub fn owners_page(
    state: State<'_, AppState>,
    generation: u64,
    filter: Option<FilterSet>,
    cursor: Option<String>,
    page_size: Option<u16>,
) -> Result<OwnerSummaryPage, UiError> {
    let snapshot = state.snapshot_for(generation)?;
    owners::owners_page(
        &snapshot,
        &filter.unwrap_or_default(),
        page_size.unwrap_or(DEFAULT_PAGE_SIZE),
        cursor.as_deref(),
    )
}

/// Returns one page of the files of a selected owner.
///
/// # Errors
/// Returns `E_NOT_FOUND` for an unknown owner and `E_STALE_CURSOR` for a
/// foreign cursor.
#[tauri::command(rename_all = "snake_case")]
#[allow(clippy::too_many_arguments)]
pub fn owners_files_page(
    state: State<'_, AppState>,
    generation: u64,
    owner: String,
    filter: Option<FilterSet>,
    metric: Option<Metric>,
    sort: Option<SortSpec>,
    cursor: Option<String>,
    page_size: Option<u16>,
) -> Result<TreePage, UiError> {
    let snapshot = state.snapshot_for(generation)?;
    owners::owners_files_page(
        &snapshot,
        &owner,
        &filter.unwrap_or_default(),
        metric.unwrap_or(Metric::Logical),
        sort.unwrap_or_default(),
        page_size.unwrap_or(DEFAULT_PAGE_SIZE),
        cursor.as_deref(),
    )
}

/// Returns one page of archive entries without extracting them.
///
/// # Errors
/// Returns `E_UNSUPPORTED` for password-protected or damaged archives and
/// `E_ACCESS_DENIED` when the archive cannot be read.
#[tauri::command(rename_all = "snake_case")]
pub fn archives_entries_page(
    task_id: Option<String>,
    path: String,
    filter: Option<FilterSet>,
    cursor: Option<String>,
    page_size: Option<u16>,
) -> Result<ArchiveEntryPage, UiError> {
    let _ = task_id;
    let path = crate::security::canonicalize(Path::new(&path))?;
    archives::entries_page(
        &path,
        &filter.unwrap_or_default(),
        page_size.unwrap_or(DEFAULT_PAGE_SIZE),
        cursor.as_deref(),
    )
}

/// Exports only the archive content list.
///
/// # Errors
/// Returns `E_UNSUPPORTED` for unsupported containers or formats,
/// `E_VALIDATION` for the output path and `E_IO` when writing fails.
#[tauri::command(rename_all = "snake_case")]
pub fn archives_export_list(
    path: String,
    format: String,
    out: String,
) -> Result<ExportResult, UiError> {
    let format = ExportFormat::parse_value(&format).ok_or_else(|| {
        UiError::new(UiErrorCode::Unsupported).with_details(format!(
            "archive.unsupported.format: unknown format `{format}`"
        ))
    })?;
    let path = crate::security::canonicalize(Path::new(&path))?;
    archives::export_list(&path, format, Path::new(&out))
}

/// Builds an export preview and its one-time token.
///
/// # Errors
/// Returns `E_VALIDATION` for an invalid composition or path and
/// `E_STALE_CURSOR` when the generation changed.
#[tauri::command(rename_all = "snake_case")]
pub fn export_preview(
    state: State<'_, AppState>,
    scope: ExportScope,
    format: ExportFormat,
    composition: Option<ExportComposition>,
) -> Result<ExportPreview, UiError> {
    let request = ExportPreviewRequest {
        scope,
        format,
        composition: composition.unwrap_or_default(),
    };
    let snapshot = state.snapshot_for(request.scope.generation)?;
    export::preview(&request, snapshot, freespace_app::operations::now_millis())
}

/// Consumes a preview and starts the export worker.
///
/// The preview identifier is one-time. The optional token is checked when the
/// client supplies it, so a contract-only caller may send just `preview_id`.
///
/// # Errors
/// Returns `E_PREVIEW_EXPIRED` for a missing, used or expired preview,
/// `E_VALIDATION` when the output file appeared meanwhile and `E_INTERNAL` when
/// the worker cannot start.
#[tauri::command(rename_all = "snake_case")]
pub fn export_start(
    preview_id: String,
    confirmation_token: Option<String>,
) -> Result<TaskStarted, UiError> {
    export::start(
        &preview_id,
        confirmation_token.as_deref().unwrap_or_default(),
        freespace_app::operations::now_millis(),
    )
}

/// Returns the polled state of an export.
///
/// # Errors
/// Returns `E_NOT_FOUND` for an unknown task.
#[tauri::command(rename_all = "snake_case")]
pub fn export_status(task_id: String) -> Result<ExportStatus, UiError> {
    export::status(&task_id)
}

/// Requests cooperative cancellation of an export.
///
/// # Errors
/// Returns `E_NOT_FOUND` for an unknown task.
#[tauri::command(rename_all = "snake_case")]
pub fn export_cancel(task_id: String) -> Result<(), UiError> {
    export::cancel(&task_id)
}

/// Lists the applications that hold a file.
///
/// # Errors
/// Returns `E_NOT_FOUND` for a missing object and `E_ACCESS_DENIED` when the
/// Restart Manager cannot query it.
#[tauri::command(rename_all = "snake_case")]
pub fn unlock_locks(path: String) -> Result<Vec<FileLock>, UiError> {
    unlock::locks(Path::new(&path))
}

/// Releases locks in soft or force mode.
///
/// The command call is the user's confirmation: the screen shows its dialog
/// before invoking it. The optional `confirmed` field exists for diagnostics and
/// defaults to true at this boundary.
///
/// # Errors
/// Returns `E_ELEVATION_REQUIRED` for force without elevation,
/// `E_ACCESS_DENIED` for critical processes and `E_BUSY` when the object stays
/// locked.
#[tauri::command(rename_all = "snake_case")]
pub fn unlock_release(
    state: State<'_, AppState>,
    path: String,
    mode: String,
    process_ids: Option<Vec<u32>>,
    confirmed: Option<bool>,
) -> Result<OperationAccepted, UiError> {
    let mode = UnlockMode::parse_value(&mode)
        .ok_or_else(|| UiError::validation(format!("unknown unlock mode `{mode}`")))?;
    let elevated = freespace_platform::elevation::is_elevated();
    let process_ids = process_ids.unwrap_or_default();
    let confirmed = confirmed.unwrap_or(true);
    let result = state.with_store(|store| {
        unlock::release(
            Path::new(&path),
            mode,
            &process_ids,
            confirmed,
            elevated,
            Some(store),
        )
    })?;

    Ok(OperationAccepted {
        operation_id: result.operation_id,
    })
}

/// Registers a file in the reboot deletion queue.
///
/// The command call is the user's confirmation; `confirmed` defaults to true at
/// this boundary and exists for diagnostics.
///
/// # Errors
/// Returns `E_ELEVATION_REQUIRED` without administrator rights and
/// `E_ACCESS_DENIED` when the pending-rename key cannot be written.
#[tauri::command(rename_all = "snake_case")]
pub fn unlock_schedule_reboot(
    state: State<'_, AppState>,
    path: String,
    confirmed: Option<bool>,
) -> Result<OperationAccepted, UiError> {
    let elevated = freespace_platform::elevation::is_elevated();
    let result = state.with_store(|store| {
        unlock::schedule_reboot(
            Path::new(&path),
            confirmed.unwrap_or(true),
            elevated,
            Some(store),
        )
    })?;

    Ok(OperationAccepted {
        operation_id: result.operation_id,
    })
}
