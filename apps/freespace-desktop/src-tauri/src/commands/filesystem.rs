//! Filesystem commands: size details, long paths and rename (US4).
//!
//! The commands resolve the current generation or the shared journal store and
//! call the `freespace-app` services. All validation, conflict detection,
//! preview tokens and file-system work live in those services; nothing here
//! touches a path directly and nothing here computes a size.

use std::path::Path;

use freespace_app::{
    dto::{
        DEFAULT_PAGE_SIZE, FilterSet, LongPathPageRequest, LongPathValidation, RenameApplied,
        RenameCase, RenamePreview, RenamePreviewRequest, SizeDetails, TreePage,
    },
    error::{UiError, UiErrorCode},
    long_paths,
    operations::now_millis,
    rename, size_details,
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

/// Opens an object of the current snapshot in Windows Explorer.
///
/// The command only reveals an existing object inside the scanned area; it
/// never changes anything. A directory opens as a folder and a file opens its
/// parent with the file selected.
#[tauri::command(rename_all = "snake_case")]
pub fn reveal_in_explorer(state: State<'_, AppState>, path: String) -> Result<(), UiError> {
    // Resolve `..` before the scope comparison, exactly like the operation
    // layer does; a path may never escape the scanned area.
    let target = freespace_platform::paths::normalize(Path::new(&path))?;
    let snapshot = state.snapshot()?;
    if !snapshot
        .root_paths()
        .iter()
        .any(|root| crate::security::is_within(root, &target))
    {
        return Err(UiError::new(UiErrorCode::AccessDenied)
            .with_details("the target is outside the scanned area"));
    }

    reveal(&target)
}

/// Opens an object in the platform file manager.
#[cfg(windows)]
fn reveal(target: &Path) -> Result<(), UiError> {
    freespace_platform::reveal::reveal(target).map_err(UiError::from)
}

/// Opening a file manager is a Windows-only shell integration for now.
#[cfg(not(windows))]
fn reveal(_target: &Path) -> Result<(), UiError> {
    Err(UiError::new(UiErrorCode::Unsupported)
        .with_details("opening the file manager is not available on this platform"))
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
    let elevated = freespace_platform::platform().elevation.is_elevated();

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
