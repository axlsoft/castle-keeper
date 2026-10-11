use tauri::State;

use crate::{
    error::AppResult,
    kafka::topics::{self, TopicConfigEntry, TopicConsumer, TopicDetail, TopicSummary},
    state::AppState,
};

/// Topics in the active cluster with partition count, replication factor
/// and health, sorted by name.
#[tauri::command]
#[specta::specta]
pub async fn list_topics(
    state: State<'_, AppState>,
    include_internal: bool,
) -> AppResult<Vec<TopicSummary>> {
    let (_profile, settings) = state.active_connection()?;
    let clients = state.clients.clients(&settings)?;
    topics::list_topics(clients, include_internal)
        .await
        .inspect_err(|error| tracing::warn!(%error, "listing topics failed"))
}

#[tauri::command]
#[specta::specta]
pub async fn describe_topic(state: State<'_, AppState>, name: String) -> AppResult<TopicDetail> {
    let (_profile, settings) = state.active_connection()?;
    topics::describe_topic(state.clients.clients(&settings)?, name).await
}

#[tauri::command]
#[specta::specta]
pub async fn topic_configs(
    state: State<'_, AppState>,
    name: String,
) -> AppResult<Vec<TopicConfigEntry>> {
    let (_profile, settings) = state.active_connection()?;
    topics::topic_configs(state.clients.clients(&settings)?, name).await
}

#[tauri::command]
#[specta::specta]
pub async fn topic_consumers(
    state: State<'_, AppState>,
    name: String,
) -> AppResult<Vec<TopicConsumer>> {
    let (_profile, settings) = state.active_connection()?;
    topics::topic_consumers(state.clients.clients(&settings)?, name).await
}
