use tauri::{Manager, State};

use crate::{
    audit::ActionEntry,
    error::{AppError, AppResult, ErrorKind},
    state::AppState,
};

/// Most recent action log entries, newest first.
#[tauri::command]
#[specta::specta]
pub fn list_actions(state: State<'_, AppState>, limit: u32) -> AppResult<Vec<ActionEntry>> {
    state.guard.log().recent(limit as usize)
}

/// Copies the full action log to the Downloads folder and returns its path.
#[tauri::command]
#[specta::specta]
pub fn export_action_log(app: tauri::AppHandle, state: State<'_, AppState>) -> AppResult<String> {
    let downloads = app
        .path()
        .download_dir()
        .map_err(|e| AppError::new(ErrorKind::Internal, format!("no Downloads folder: {e}")))?;
    let stamp = crate::audit::now_rfc3339().replace(':', "-");
    let destination = downloads.join(format!("castle-keeper-actions-{stamp}.jsonl"));
    let source = state.guard.log().path();
    if source.exists() {
        std::fs::copy(source, &destination)
    } else {
        std::fs::write(&destination, b"").map(|_| 0)
    }
    .map_err(|e| AppError::internal(format!("could not export the action log: {e}")))?;
    Ok(destination.display().to_string())
}

/// Lifts read-only for the current session after the profile name is typed.
#[tauri::command]
#[specta::specta]
pub fn unlock_profile(
    state: State<'_, AppState>,
    id: String,
    confirmation: String,
) -> AppResult<()> {
    let profile = state.profiles.get(&id)?;
    state.guard.unlock_session(&profile, &confirmation)?;
    tracing::warn!(profile = %profile.name, environment = ?profile.environment, "read-only lifted for this session");
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn lock_profile(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.guard.session().lock(&id);
    Ok(())
}
