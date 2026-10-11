use serde::Serialize;
use specta::Type;
use tauri::State;

use crate::{
    error::AppResult,
    kafka::connection::{self, ConnectionReport},
    profiles::{Profile, ProfileInput},
    state::{take_passwords, AppState},
};

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProfileList {
    pub profiles: Vec<Profile>,
    pub active_id: Option<String>,
    /// Profiles whose read-only flag is lifted for this session.
    pub unlocked_ids: Vec<String>,
}

#[tauri::command]
#[specta::specta]
pub fn list_profiles(state: State<'_, AppState>) -> AppResult<ProfileList> {
    let profiles = state.profiles.list()?;
    let unlocked_ids = profiles
        .iter()
        .filter(|p| state.guard.session().is_unlocked(&p.id))
        .map(|p| p.id.clone())
        .collect();
    Ok(ProfileList {
        profiles,
        active_id: state.profiles.active()?.map(|p| p.id),
        unlocked_ids,
    })
}

#[tauri::command]
#[specta::specta]
pub fn create_profile(state: State<'_, AppState>, input: ProfileInput) -> AppResult<Profile> {
    let profile = state.create_profile(input)?;
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
    let profile = state.update_profile(&id, input)?;
    tracing::info!(profile = %profile.name, "connection updated");
    Ok(profile)
}

#[tauri::command]
#[specta::specta]
pub fn delete_profile(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let removed = state.delete_profile(&id)?;
    tracing::info!(profile = %removed.name, "connection deleted");
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn select_profile(state: State<'_, AppState>, id: String) -> AppResult<Profile> {
    let profile = state.select_profile(&id)?;
    tracing::info!(profile = %profile.name, "connection selected");
    Ok(profile)
}

/// Connects with unsaved settings and reports what was found. When editing
/// an existing connection (`profile_id`), its stored passwords are used for
/// any password field left untouched.
#[tauri::command]
#[specta::specta]
pub async fn test_connection(
    state: State<'_, AppState>,
    mut input: ProfileInput,
    profile_id: Option<String>,
) -> AppResult<ConnectionReport> {
    input.validate()?;
    let typed = take_passwords(&mut input);
    let profile = input.preview();
    let settings = state.settings_for(&profile, profile_id.as_deref(), &typed)?;
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
