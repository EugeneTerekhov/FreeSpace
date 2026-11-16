//! Filesystem commands: size details, long paths, rename, permissions and
//! attributes (US4, T098–T101).
//!
//! The commands resolve the current generation or the shared journal store and
//! call the `freespace-app` services. All validation, conflict detection,
//! preview tokens and file-system work live in those services; nothing here
//! touches a path directly and nothing here computes a size.

use freespace_app::{
    dto::{
        AttributeChangeSet, AttributesPreviewRequest, DEFAULT_PAGE_SIZE, FileOperationApplied,
        FilterSet, LongPathPageRequest, LongPathValidation, OperationPreview, PermissionsChanges,
        PermissionsPreview, PermissionsPreviewRequest, PermissionsView, RenameApplied, RenameCase,
        RenamePreview, RenamePreviewRequest, SizeDetails, TreePage,
    },
    error::{UiError, UiErrorCode},
    long_paths,
    operations::now_millis,
    permissions, rename, size_details,
};
use tauri::State;

use crate::state::AppState;

/// Returns the exact size breakdown of one node.
#[tauri::command(rename_all = "snake_case")]
pub fn size_details(
    state: State<'_, AppState>,
    generation: u64,
    node_id: u32,
) -> Result<SizeDetails, UiError> {
    let snapshot = state.snapshot_for(generation).map_err(|error| {
        if error.code == UiErrorCode::StaleCursor {
            UiError::new(UiErrorCode::StaleNode)
                .with_details("the requested generation is no longer current")
        } else {
            error
        }
    })?;

    size_details::size_details(&snapshot, node_id)
}

/// Returns one page of the objects whose path exceeds `MAX_PATH`.
#[tauri::command(rename_all = "snake_case")]
pub fn long_paths_page(
    state: State<'_, AppState>,
    generation: u64,
    filter: Option<FilterSet>,
    cursor: Option<String>,
    page_size: Option<u16>,
) -> Result<TreePage, UiError> {
    let snapshot = state.snapshot_for(generation)?;
    let request = LongPathPageRequest {
        generation,
        filter: filter.unwrap_or_default(),
        cursor,
        page_size: page_size.unwrap_or(DEFAULT_PAGE_SIZE),
    };

    long_paths::long_paths_page(&snapshot, &request)
}

/// Validates a long path and reports its canonical verbatim form.
#[tauri::command(rename_all = "snake_case")]
pub fn long_paths_validate(path: String) -> Result<LongPathValidation, UiError> {
    long_paths::validate(&path)
}

/// Builds a bulk-rename plan and stores its one-time token.
#[tauri::command(rename_all = "snake_case")]
pub fn rename_preview(
    targets: Vec<String>,
    regex: String,
    replacement: Option<String>,
    scope: Option<String>,
    case: Option<RenameCase>,
) -> Result<RenamePreview, UiError> {
    let request = RenamePreviewRequest {
        targets,
        regex,
        replacement: replacement.unwrap_or_default(),
        scope,
        case: case.unwrap_or_default(),
    };

    rename::preview(&request, now_millis())
}

/// Consumes a rename preview and applies it.
#[tauri::command(rename_all = "snake_case")]
pub fn rename_apply(
    state: State<'_, AppState>,
    preview_id: String,
    confirmation_token: String,
) -> Result<RenameApplied, UiError> {
    let elevated = freespace_platform::elevation::is_elevated();

    state.with_store(|store| {
        rename::apply(
            &preview_id,
            &confirmation_token,
            Some(store),
            elevated,
            now_millis(),
        )
    })
}

/// Reads the owner, attributes and protection state of one object.
#[tauri::command(rename_all = "snake_case")]
pub fn permissions_get(path: String) -> Result<PermissionsView, UiError> {
    permissions::permissions_view(&path)
}

/// Builds a permission preview without changing anything.
#[tauri::command(rename_all = "snake_case")]
pub fn permissions_preview(
    path: String,
    changes: PermissionsChanges,
) -> Result<PermissionsPreview, UiError> {
    let request = PermissionsPreviewRequest { path, changes };

    permissions::preview(&request, now_millis())
}

/// Consumes a permission preview and applies its changes.
#[tauri::command(rename_all = "snake_case")]
pub fn permissions_apply(
    state: State<'_, AppState>,
    preview_id: String,
    confirmation_token: String,
) -> Result<FileOperationApplied, UiError> {
    let elevated = freespace_platform::elevation::is_elevated();

    state.with_store(|store| {
        permissions::apply(
            &preview_id,
            &confirmation_token,
            Some(store),
            elevated,
            now_millis(),
        )
    })
}

/// Builds a batch attribute preview in the shared operation-preview shape.
#[tauri::command(rename_all = "snake_case")]
pub fn attributes_preview(
    targets: Vec<String>,
    changes: AttributeChangeSet,
) -> Result<OperationPreview, UiError> {
    let request = AttributesPreviewRequest { targets, changes };

    permissions::attributes_preview(&request, now_millis())
}

/// Consumes an attribute preview and applies it.
#[tauri::command(rename_all = "snake_case")]
pub fn attributes_apply(
    state: State<'_, AppState>,
    preview_id: String,
    confirmation_token: String,
) -> Result<FileOperationApplied, UiError> {
    let elevated = freespace_platform::elevation::is_elevated();

    state.with_store(|store| {
        permissions::attributes_apply(
            &preview_id,
            &confirmation_token,
            Some(store),
            elevated,
            now_millis(),
        )
    })
}
