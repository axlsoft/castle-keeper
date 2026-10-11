use tauri::State;

use crate::{error::AppResult, kafka::topics, state::AppState};

/// Topic names in the active cluster, sorted.
#[tauri::command]
#[specta::specta]
pub async fn list_topics(
    state: State<'_, AppState>,
    include_internal: bool,
) -> AppResult<Vec<String>> {
    let (_profile, settings) = state.active_connection()?;
    let clients = state.clients.clients(&settings)?;
    topics::list_topic_names(clients, include_internal)
        .await
        .inspect_err(|error| tracing::warn!(%error, "listing topics failed"))
}
