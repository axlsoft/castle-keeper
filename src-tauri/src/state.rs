use std::{
    collections::HashMap,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use crate::{
    audit::ActionLog,
    error::{AppError, AppResult, ErrorKind},
    guard::{SessionLocks, WriteGuard},
    kafka::{manager::ClientManager, settings::ConnectionSettings},
    profiles::{Profile, ProfileInput, ProfileSecrets, ProfileStore},
    secrets::{KeyringStore, SecretField, SecretStore},
};

/// Long-lived backend state shared by all commands.
pub struct AppState {
    pub clients: ClientManager,
    pub profiles: ProfileStore,
    pub secrets: Arc<dyn SecretStore>,
    pub guard: WriteGuard,
    pub browses: Browses,
}

/// Cancellation flags for running message browses, by browse id.
#[derive(Default)]
pub struct Browses(Mutex<HashMap<String, Arc<AtomicBool>>>);

impl Browses {
    pub fn insert(&self, id: &str, flag: Arc<AtomicBool>) -> AppResult<()> {
        self.0
            .lock()
            .map_err(|_| AppError::internal("browse registry lock poisoned"))?
            .insert(id.to_string(), flag);
        Ok(())
    }

    pub fn remove(&self, id: &str) {
        if let Ok(mut map) = self.0.lock() {
            map.remove(id);
        }
    }

    pub fn cancel(&self, id: &str) {
        if let Ok(map) = self.0.lock() {
            if let Some(flag) = map.get(id) {
                flag.store(true, Ordering::Relaxed);
            }
        }
    }
}

impl AppState {
    /// Loads state from the app config directory. A corrupt profile file is
    /// moved aside so the app still starts.
    pub fn load(config_dir: &Path, data_dir: &Path) -> Self {
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
        let log = ActionLog::new(data_dir.join("actions.jsonl"));
        Self::new(profiles, Arc::new(KeyringStore), log)
    }

    pub fn new(profiles: ProfileStore, secrets: Arc<dyn SecretStore>, log: ActionLog) -> Self {
        Self {
            clients: ClientManager::default(),
            profiles,
            secrets,
            guard: WriteGuard::new(Arc::new(log), Arc::new(SessionLocks::default())),
            browses: Browses::default(),
        }
    }

    /// Switches the active profile. Any session unlock is revoked.
    pub fn select_profile(&self, id: &str) -> AppResult<Profile> {
        let profile = self.profiles.set_active(id)?;
        self.guard.session().lock_all();
        Ok(profile)
    }

    pub fn create_profile(&self, mut input: ProfileInput) -> AppResult<Profile> {
        let passwords = take_passwords(&mut input);
        let profile = self.profiles.create(input)?;
        if let Err(error) = self.apply_passwords(&profile.id, passwords) {
            // Don't leave a profile behind whose secrets were never saved.
            let _ = self.profiles.delete(&profile.id);
            return Err(error);
        }
        Ok(profile)
    }

    pub fn update_profile(&self, id: &str, mut input: ProfileInput) -> AppResult<Profile> {
        let passwords = take_passwords(&mut input);
        let profile = self.profiles.update(id, input)?;
        self.apply_passwords(id, passwords)?;
        self.clients.evict(id);
        Ok(profile)
    }

    pub fn delete_profile(&self, id: &str) -> AppResult<Profile> {
        let removed = self.profiles.delete(id)?;
        self.guard.session().lock(id);
        self.clients.evict(id);
        if let Err(error) = self.secrets.delete_all(id) {
            tracing::warn!(%error, profile = %removed.name, "could not remove stored secrets");
        }
        Ok(removed)
    }

    /// Settings for a saved or unsaved profile. Passwords typed into the form
    /// win; otherwise the stored ones for `stored_id` are used.
    pub fn settings_for(
        &self,
        profile: &Profile,
        stored_id: Option<&str>,
        typed: &Passwords,
    ) -> AppResult<ConnectionSettings> {
        let stored = match stored_id {
            Some(id) => self.secrets.load(id)?,
            None => ProfileSecrets::default(),
        };
        let pick = |typed: &Option<String>, stored: Option<String>| match typed {
            Some(value) if value.is_empty() => None,
            Some(value) => Some(value.clone()),
            None => stored,
        };
        let secrets = ProfileSecrets {
            sasl_password: pick(&typed.sasl, stored.sasl_password),
            tls_key_password: pick(&typed.tls_key, stored.tls_key_password),
        };
        Ok(profile.connection_settings(&secrets))
    }

    /// The selected profile and the settings to reach it.
    pub fn active_connection(&self) -> AppResult<(Profile, ConnectionSettings)> {
        let profile = self.profiles.active()?.ok_or_else(|| {
            AppError::new(ErrorKind::NotFound, "No connection is selected.")
                .with_hint("Add a connection on the Connections page.")
        })?;
        let settings = self.settings_for(&profile, Some(&profile.id), &Passwords::default())?;
        Ok((profile, settings))
    }

    fn apply_passwords(&self, id: &str, passwords: Passwords) -> AppResult<()> {
        for (field, value) in [
            (SecretField::SaslPassword, passwords.sasl),
            (SecretField::TlsKeyPassword, passwords.tls_key),
        ] {
            match value {
                None => {}
                Some(value) if value.is_empty() => self.secrets.delete(id, field)?,
                Some(value) => self.secrets.set(id, field, &value)?,
            }
        }
        Ok(())
    }
}

/// Passwords typed into the connection form, kept apart from the profile.
#[derive(Debug, Default, Clone)]
pub struct Passwords {
    pub sasl: Option<String>,
    pub tls_key: Option<String>,
}

pub fn take_passwords(input: &mut ProfileInput) -> Passwords {
    Passwords {
        sasl: input.sasl_password.take(),
        tls_key: input.tls_key_password.take(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        profiles::{Environment, SaslConfig, SaslMechanism, TlsConfig},
        secrets::MemoryStore,
    };

    fn input(password: Option<&str>) -> ProfileInput {
        ProfileInput {
            name: "secure".into(),
            environment: Environment::Dev,
            bootstrap: "localhost:9094".into(),
            sasl: Some(SaslConfig {
                mechanism: SaslMechanism::ScramSha256,
                username: "alice".into(),
            }),
            tls: TlsConfig::default(),
            read_only: false,
            admin_timeout_ms: None,
            delivery_timeout_ms: None,
            sasl_password: password.map(str::to_string),
            tls_key_password: None,
        }
    }

    fn state() -> (tempfile::TempDir, Arc<MemoryStore>, AppState) {
        let dir = tempfile::tempdir().unwrap();
        let secrets = Arc::new(MemoryStore::default());
        let store = ProfileStore::load(dir.path().join("profiles.json")).unwrap();
        let log = ActionLog::new(dir.path().join("actions.jsonl"));
        let state = AppState::new(store, secrets.clone(), log);
        (dir, secrets, state)
    }

    fn sasl_password(settings: &ConnectionSettings) -> Option<String> {
        settings
            .properties
            .iter()
            .find(|(k, _)| k == "sasl.password")
            .map(|(_, v)| v.clone())
    }

    #[test]
    fn passwords_go_to_the_secret_store_not_the_profile_file() {
        let (dir, secrets, state) = state();
        let profile = state.create_profile(input(Some("alice-secret"))).unwrap();

        let on_disk = std::fs::read_to_string(dir.path().join("profiles.json")).unwrap();
        assert!(!on_disk.contains("alice-secret"));
        assert_eq!(
            secrets
                .get(&profile.id, SecretField::SaslPassword)
                .unwrap()
                .as_deref(),
            Some("alice-secret")
        );

        let (_, settings) = state.active_connection().unwrap();
        assert_eq!(sasl_password(&settings).as_deref(), Some("alice-secret"));
    }

    #[test]
    fn update_keeps_replaces_or_clears_the_password() {
        let (_dir, secrets, state) = state();
        let id = state.create_profile(input(Some("one"))).unwrap().id;
        let stored = || secrets.get(&id, SecretField::SaslPassword).unwrap();

        state.update_profile(&id, input(None)).unwrap();
        assert_eq!(stored().as_deref(), Some("one"));
        state.update_profile(&id, input(Some("two"))).unwrap();
        assert_eq!(stored().as_deref(), Some("two"));
        state.update_profile(&id, input(Some(""))).unwrap();
        assert_eq!(stored(), None);
    }

    #[test]
    fn delete_removes_stored_secrets() {
        let (_dir, secrets, state) = state();
        let id = state.create_profile(input(Some("gone"))).unwrap().id;
        state.delete_profile(&id).unwrap();
        assert_eq!(secrets.load(&id).unwrap(), ProfileSecrets::default());
    }

    #[test]
    fn typed_password_overrides_stored_one_for_tests() {
        let (_dir, _secrets, state) = state();
        let profile = state.create_profile(input(Some("stored"))).unwrap();
        let typed = Passwords {
            sasl: Some("typed".into()),
            tls_key: None,
        };
        let settings = state
            .settings_for(&profile, Some(&profile.id), &typed)
            .unwrap();
        assert_eq!(sasl_password(&settings).as_deref(), Some("typed"));
        let settings = state
            .settings_for(&profile, Some(&profile.id), &Passwords::default())
            .unwrap();
        assert_eq!(sasl_password(&settings).as_deref(), Some("stored"));
    }
}
