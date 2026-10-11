use tauri::State;

use crate::{error::AppResult, kafka::topics, state::AppState};

/// Topic names in the active cluster, sorted.
#[tauri::command]
#[specta::specta]
pub async fn list_topics(
    state: State<'_, AppState>,
    include_internal: bool,
) -> AppResult<Vec<String>> {
    let clients = state.clients.clients(&state.active_connection())?;
    topics::list_topic_names(clients, include_internal)
        .await
        .inspect_err(|error| tracing::warn!(%error, "listing topics failed"))
}
