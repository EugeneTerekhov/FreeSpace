//! Volume commands used by the dashboard and the scan screen.

use tauri::State;

use freespace_app::{
    dto::{VolumeDetail, VolumeSummary},
    error::UiError,
    volumes,
};

use crate::state::AppState;

/// Lists every logical drive.
#[tauri::command(rename_all = "snake_case")]
pub fn volumes_list() -> Result<Vec<VolumeSummary>, UiError> {
    volumes::list_volume_summaries()
}

/// Re-enumerates every logical drive.
#[tauri::command(rename_all = "snake_case")]
pub fn volumes_refresh() -> Result<Vec<VolumeSummary>, UiError> {
    volumes::list_volume_summaries()
}

/// Returns the detailed summary of one volume.
#[tauri::command(rename_all = "snake_case")]
pub fn volumes_summary(volume_id: String) -> Result<VolumeDetail, UiError> {
    volumes::volume_detail(&volume_id)
}

/// Returns the volume that contains the current snapshot.
#[tauri::command(rename_all = "snake_case")]
pub fn volumes_current(state: State<'_, AppState>) -> Result<VolumeSummary, UiError> {
    let snapshot = state.snapshot()?;

    Ok(volumes::summary(snapshot.volume()))
}

/// Returns every volume of a snapshot generation, in root order.
#[tauri::command(rename_all = "snake_case")]
pub fn snapshot_volumes(
    state: State<'_, AppState>,
    generation: u64,
) -> Result<Vec<VolumeSummary>, UiError> {
    let snapshot = state.snapshot_for(generation)?;

    Ok(snapshot.volumes().iter().map(volumes::summary).collect())
}
