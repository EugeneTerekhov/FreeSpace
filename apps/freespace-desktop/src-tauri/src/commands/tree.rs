//! Tree and search commands. Pages are bound to a generation and validated by
//! `freespace-app`; the command only resolves the snapshot.

use tauri::State;

use freespace_app::{
    dto::{DEFAULT_PAGE_SIZE, FilterSet, Metric, NodeRow, SortSpec, TreePage},
    error::UiError,
};

use crate::state::AppState;

/// Returns the first page of scan roots.
#[tauri::command(rename_all = "snake_case")]
pub fn tree_root(
    state: State<'_, AppState>,
    generation: u64,
    metric: Option<Metric>,
    sort: Option<SortSpec>,
    page_size: Option<u16>,
    cursor: Option<String>,
) -> Result<TreePage, UiError> {
    let snapshot = state.snapshot_for(generation)?;

    snapshot.root_page(
        metric.unwrap_or_default(),
        sort.unwrap_or_default(),
        page_size.unwrap_or(DEFAULT_PAGE_SIZE),
        cursor.as_deref(),
    )
}

/// Returns one page of children.
#[tauri::command(rename_all = "snake_case")]
#[allow(clippy::too_many_arguments)]
pub fn tree_children(
    state: State<'_, AppState>,
    generation: u64,
    parent_id: u32,
    metric: Option<Metric>,
    sort: Option<SortSpec>,
    filter: Option<FilterSet>,
    page_size: Option<u16>,
    cursor: Option<String>,
) -> Result<TreePage, UiError> {
    let snapshot = state.snapshot_for(generation)?;

    snapshot.children_page(
        parent_id,
        metric.unwrap_or_default(),
        sort.unwrap_or_default(),
        &filter.unwrap_or_default(),
        page_size.unwrap_or(DEFAULT_PAGE_SIZE),
        cursor.as_deref(),
    )
}

/// Searches every live node by name.
#[tauri::command(rename_all = "snake_case")]
#[allow(clippy::too_many_arguments)]
pub fn tree_search(
    state: State<'_, AppState>,
    generation: u64,
    query: String,
    metric: Option<Metric>,
    sort: Option<SortSpec>,
    filter: Option<FilterSet>,
    page_size: Option<u16>,
    cursor: Option<String>,
) -> Result<TreePage, UiError> {
    let snapshot = state.snapshot_for(generation)?;

    snapshot.search_page(
        &query,
        metric.unwrap_or_default(),
        sort.unwrap_or_default(),
        &filter.unwrap_or_default(),
        page_size.unwrap_or(DEFAULT_PAGE_SIZE),
        cursor.as_deref(),
    )
}

/// Returns one node in the current generation.
#[tauri::command(rename_all = "snake_case")]
pub fn tree_node(
    state: State<'_, AppState>,
    generation: u64,
    node_id: u32,
) -> Result<NodeRow, UiError> {
    state.snapshot_for(generation)?.node_row(node_id)
}

/// Returns the ancestors of a node from the root to its parent.
#[tauri::command(rename_all = "snake_case")]
pub fn tree_path(
    state: State<'_, AppState>,
    generation: u64,
    node_id: u32,
) -> Result<Vec<NodeRow>, UiError> {
    state.snapshot_for(generation)?.path_rows(node_id)
}
