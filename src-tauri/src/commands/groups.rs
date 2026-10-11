use tauri::State;

use crate::{
    error::AppResult,
    kafka::groups::{self, GroupDetail, GroupSummary},
    state::AppState,
};

/// Every consumer group in the active cluster with state, members, topics
/// and total lag.
#[tauri::command]
#[specta::specta]
pub async fn list_groups(state: State<'_, AppState>) -> AppResult<Vec<GroupSummary>> {
    let (_profile, settings) = state.active_connection()?;
    groups::list_groups(state.clients.clients(&settings)?)
        .await
        .inspect_err(|error| tracing::warn!(%error, "listing consumer groups failed"))
}

/// Members and per-partition committed offset, log end offset and lag.
#[tauri::command]
#[specta::specta]
pub async fn describe_group(
    state: State<'_, AppState>,
    group_id: String,
) -> AppResult<GroupDetail> {
    let (_profile, settings) = state.active_connection()?;
    groups::describe_group(state.clients.clients(&settings)?, group_id).await
}
