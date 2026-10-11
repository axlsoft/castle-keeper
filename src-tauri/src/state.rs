use std::path::Path;

use crate::{
    error::{AppError, AppResult, ErrorKind},
    kafka::{manager::ClientManager, settings::ConnectionSettings},
    profiles::{Profile, ProfileSecrets, ProfileStore},
};

/// Long-lived backend state shared by all commands.
pub struct AppState {
    pub clients: ClientManager,
    pub profiles: ProfileStore,
}

impl AppState {
    /// Loads state from the app config directory. A corrupt profile file is
    /// moved aside so the app still starts.
    pub fn load(config_dir: &Path) -> Self {
        let path = config_dir.join("profiles.json");
        let profiles = match ProfileStore::load(&path) {
            Ok(store) => store,
            Err(error) => {
                let backup = path.with_extension(format!("json.corrupt-{}", std::process::id()));
                tracing::error!(%error, backup = %backup.display(), "profile file unreadable; starting empty");
                let _ = std::fs::rename(&path, &backup);
                ProfileStore::empty(path)
            }
        };
        Self {
            clients: ClientManager::default(),
            profiles,
        }
    }

    /// The selected profile and the settings to reach it.
    pub fn active_connection(&self) -> AppResult<(Profile, ConnectionSettings)> {
        let profile = self.profiles.active()?.ok_or_else(|| {
            AppError::new(ErrorKind::NotFound, "No connection is selected.")
                .with_hint("Add a connection on the Connections page.")
        })?;
        let settings = profile.connection_settings(&ProfileSecrets::default());
        Ok((profile, settings))
    }
}
