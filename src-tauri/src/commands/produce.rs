use tauri::State;

use crate::{
    audit::WriteAction,
    error::AppResult,
    guard::WriteRequest,
    kafka::produce::{self, ProduceOutcome, ProduceRequest},
    state::AppState,
};

/// Produces one message to the active connection, subject to the write
/// guard. `dry_run` validates and previews without sending.
#[tauri::command]
#[specta::specta]
pub async fn produce_message(
    state: State<'_, AppState>,
    request: ProduceRequest,
    dry_run: bool,
    confirmation: Option<String>,
) -> AppResult<ProduceOutcome> {
    let (profile, settings) = state.active_connection()?;
    let ticket = state
        .guard
        .authorize(
            &profile,
            WriteRequest {
                action: WriteAction::Produce,
                target: request.topic.clone(),
                dry_run,
                confirmation,
            },
        )
        .inspect_err(|error| {
            tracing::warn!(profile = %profile.name, topic = %request.topic, %error, "produce blocked")
        })?;
    let clients = state.clients.clients(&settings)?;
    let result = produce::produce(&clients, &request, ticket).await;
    match &result {
        Ok(outcome) => tracing::info!(
            profile = %profile.name,
            topic = %request.topic,
            dry_run = outcome.dry_run,
            partition = ?outcome.partition,
            offset = ?outcome.offset,
            "produce finished"
        ),
        Err(error) => tracing::warn!(topic = %request.topic, %error, "produce failed"),
    }
    result
}
