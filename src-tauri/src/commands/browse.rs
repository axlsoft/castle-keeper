use std::sync::{Arc, atomic::AtomicBool};

use tauri::{State, ipc::Channel};

use crate::{
    error::{AppError, AppResult},
    kafka::browse::{self, BrowseEvent, BrowseRequest},
    state::AppState,
};

/// Streams messages from the active connection to `channel` in batches.
/// Read-only: uses a consumer without a group id. `browse_id` lets the UI
/// cancel it with `cancel_browse`.
#[tauri::command]
#[specta::specta]
pub async fn browse_messages(
    state: State<'_, AppState>,
    browse_id: String,
    request: BrowseRequest,
    channel: Channel<BrowseEvent>,
) -> AppResult<()> {
    let (_profile, settings) = state.active_connection()?;
    let clients = state.clients.clients(&settings)?;
    let cancel = Arc::new(AtomicBool::new(false));
    state.browses.insert(&browse_id, cancel.clone())?;

    tracing::info!(topic = %request.topic, start = ?request.start, limit = request.limit, "browse started");
    let result = browse::browse_async(clients, request, cancel, move |event| {
        channel
            .send(event)
            .map_err(|e| AppError::internal(format!("could not send messages to the UI: {e}")))
    })
    .await;
    state.browses.remove(&browse_id);
    if let Err(error) = &result {
        tracing::warn!(%error, "browse failed");
    }
    result
}

#[tauri::command]
#[specta::specta]
pub fn cancel_browse(state: State<'_, AppState>, browse_id: String) {
    state.browses.cancel(&browse_id);
}
