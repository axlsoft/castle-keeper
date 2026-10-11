use tauri::State;

use crate::{
    error::AppResult,
    kafka::produce::{self, ProduceRequest, ProduceResult},
    state::AppState,
};

#[tauri::command]
#[specta::specta]
pub async fn produce_message(
    state: State<'_, AppState>,
    request: ProduceRequest,
) -> AppResult<ProduceResult> {
    let clients = state.clients.clients(&state.active_connection())?;
    let result = produce::produce(&clients, &request).await;
    match &result {
        Ok(delivered) => tracing::info!(
            topic = %request.topic,
            partition = delivered.partition,
            offset = delivered.offset,
            "message produced"
        ),
        Err(error) => tracing::warn!(topic = %request.topic, %error, "produce failed"),
    }
    result
}
