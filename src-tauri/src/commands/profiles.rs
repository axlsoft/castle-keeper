use serde::Serialize;
use specta::Type;
use tauri::State;

use crate::{
    error::AppResult,
    kafka::connection::{self, ConnectionReport},
    profiles::{Profile, ProfileInput, ProfileSecrets},
    state::AppState,
};

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProfileList {
    pub profiles: Vec<Profile>,
    pub active_id: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub fn list_profiles(state: State<'_, AppState>) -> AppResult<ProfileList> {
    Ok(ProfileList {
        profiles: state.profiles.list()?,
        active_id: state.profiles.active()?.map(|p| p.id),
    })
}

#[tauri::command]
#[specta::specta]
pub fn create_profile(state: State<'_, AppState>, input: ProfileInput) -> AppResult<Profile> {
    let profile = state.profiles.create(input)?;
    tracing::info!(profile = %profile.name, environment = ?profile.environment, "connection created");
    Ok(profile)
}

#[tauri::command]
#[specta::specta]
pub fn update_profile(
    state: State<'_, AppState>,
    id: String,
    input: ProfileInput,
) -> AppResult<Profile> {
    let profile = state.profiles.update(&id, input)?;
    state.clients.evict(&id);
    tracing::info!(profile = %profile.name, "connection updated");
    Ok(profile)
}

#[tauri::command]
#[specta::specta]
pub fn delete_profile(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let removed = state.profiles.delete(&id)?;
    state.clients.evict(&id);
    tracing::info!(profile = %removed.name, "connection deleted");
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn select_profile(state: State<'_, AppState>, id: String) -> AppResult<Profile> {
    let profile = state.profiles.set_active(&id)?;
    tracing::info!(profile = %profile.name, "connection selected");
    Ok(profile)
}

/// Connects with unsaved settings and reports what was found.
#[tauri::command]
#[specta::specta]
pub async fn test_connection(input: ProfileInput) -> AppResult<ConnectionReport> {
    input.validate()?;
    let profile = input.preview();
    let settings = profile.connection_settings(&ProfileSecrets::default());
    let result = connection::test_connection(settings).await;
    match &result {
        Ok(report) => tracing::info!(
            bootstrap = %profile.bootstrap,
            brokers = report.brokers.len(),
            connect_ms = report.connect_ms,
            "connection test succeeded"
        ),
        Err(error) => {
            tracing::info!(bootstrap = %profile.bootstrap, %error, "connection test failed")
        }
    }
    result
}
