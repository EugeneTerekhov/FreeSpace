//! Treemap commands. Tiles are produced by `freespace-app` from the immutable
//! snapshot; the command applies defaults and resolves the generation.

use tauri::State;

use freespace_app::{
    dto::{Metric, NodeRow, TreemapRequest, TreemapResponse},
    error::UiError,
    treemap,
};

use crate::state::AppState;

/// Builds tiles for one viewport.
#[allow(clippy::too_many_arguments)]
#[tauri::command(rename_all = "snake_case")]
pub fn treemap_tiles(
    state: State<'_, AppState>,
    generation: u64,
    node_id: Option<u32>,
    width: u32,
    height: u32,
    metric: Option<Metric>,
    max_depth: Option<u8>,
    min_tile_px: Option<u16>,
    max_tiles: Option<u16>,
) -> Result<TreemapResponse, UiError> {
    let snapshot = state.snapshot_for(generation)?;
    let defaults = TreemapRequest::default();
    let request = TreemapRequest {
        node_id,
        width,
        height,
        metric: metric.unwrap_or(defaults.metric),
        max_depth: max_depth.unwrap_or(defaults.max_depth),
        min_tile_px: min_tile_px.unwrap_or(defaults.min_tile_px),
        max_tiles: max_tiles.unwrap_or(defaults.max_tiles),
    };

    treemap::tiles(&snapshot, &request)
}

/// Returns the node behind a tile.
#[tauri::command(rename_all = "snake_case")]
pub fn treemap_node(
    state: State<'_, AppState>,
    generation: u64,
    node_id: u32,
) -> Result<NodeRow, UiError> {
    state.snapshot_for(generation)?.node_row(node_id)
}
