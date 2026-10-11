//! Cluster connection profiles: everything about a connection except its
//! secrets, persisted as JSON in the app config directory.

use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    error::{AppError, AppResult, ErrorKind},
    kafka::settings::ConnectionSettings,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Environment {
    Local,
    Dev,
    Staging,
    Prod,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SaslMechanism {
    Plain,
    ScramSha256,
    ScramSha512,
}

impl SaslMechanism {
    pub fn librdkafka_name(self) -> &'static str {
        match self {
            SaslMechanism::Plain => "PLAIN",
            SaslMechanism::ScramSha256 => "SCRAM-SHA-256",
            SaslMechanism::ScramSha512 => "SCRAM-SHA-512",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SaslConfig {
    pub mechanism: SaslMechanism,
    pub username: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TlsConfig {
    pub enabled: bool,
    /// PEM file with the CA certificate(s); the OS trust store when empty.
    #[serde(default)]
    pub ca_path: Option<String>,
    /// Client certificate and key for mTLS.
    #[serde(default)]
    pub cert_path: Option<String>,
    #[serde(default)]
    pub key_path: Option<String>,
    #[serde(default = "default_true")]
    pub verify_hostname: bool,
}

fn default_true() -> bool {
    true
}

impl Default for TlsConfig {
    /// TLS off, but hostname verification on whenever it is enabled.
    fn default() -> Self {
        Self {
            enabled: false,
            ca_path: None,
            cert_path: None,
            key_path: None,
            verify_hostname: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub environment: Environment,
    pub bootstrap: String,
    #[serde(default)]
    pub sasl: Option<SaslConfig>,
    #[serde(default)]
    pub tls: TlsConfig,
    /// Persisted read-only flag. Always true for production profiles; a
    /// production profile can only be unlocked for the current session.
    pub read_only: bool,
    pub admin_timeout_ms: u32,
    pub delivery_timeout_ms: u32,
}

/// What the UI sends to create or update a profile.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProfileInput {
    pub name: String,
    pub environment: Environment,
    pub bootstrap: String,
    #[serde(default)]
    pub sasl: Option<SaslConfig>,
    #[serde(default)]
    pub tls: TlsConfig,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub admin_timeout_ms: Option<u32>,
    #[serde(default)]
    pub delivery_timeout_ms: Option<u32>,
}

impl ProfileInput {
    pub fn validate(&self) -> AppResult<()> {
        if self.name.trim().is_empty() {
            return Err(AppError::invalid_input("Give the connection a name."));
        }
        if self.bootstrap.trim().is_empty() {
            return Err(
                AppError::invalid_input("Bootstrap servers are required.").with_hint(
                    "Use host:port, comma-separated for several brokers, e.g. localhost:9092.",
                ),
            );
        }
        for server in self.bootstrap.split(',') {
            let server = server.trim();
            let port_ok = server
                .rsplit_once(':')
                .map(|(host, port)| !host.is_empty() && port.parse::<u16>().is_ok())
                .unwrap_or(false);
            if !port_ok {
                return Err(
                    AppError::invalid_input(format!("\"{server}\" is not host:port.")).with_hint(
                        "Use host:port, comma-separated for several brokers, e.g. localhost:9092.",
                    ),
                );
            }
        }
        if let Some(sasl) = &self.sasl {
            if sasl.username.trim().is_empty() {
                return Err(AppError::invalid_input("SASL needs a username."));
            }
        }
        Ok(())
    }

    /// The profile these settings would produce, for testing before saving.
    pub fn preview(self) -> Profile {
        self.into_profile("connection-test".into())
    }

    fn into_profile(self, id: String) -> Profile {
        Profile {
            id,
            name: self.name.trim().to_string(),
            environment: self.environment,
            bootstrap: self
                .bootstrap
                .split(',')
                .map(str::trim)
                .collect::<Vec<_>>()
                .join(","),
            sasl: self.sasl,
            tls: self.tls,
            // Production is always read-only at rest.
            read_only: self.read_only || self.environment == Environment::Prod,
            admin_timeout_ms: self.admin_timeout_ms.unwrap_or(10_000).max(1_000),
            delivery_timeout_ms: self.delivery_timeout_ms.unwrap_or(15_000).max(1_000),
        }
    }
}

impl Profile {
    pub fn security_protocol(&self) -> &'static str {
        match (self.sasl.is_some(), self.tls.enabled) {
            (false, false) => "PLAINTEXT",
            (false, true) => "SSL",
            (true, false) => "SASL_PLAINTEXT",
            (true, true) => "SASL_SSL",
        }
    }

    /// librdkafka settings for this profile. Secrets are passed in, never
    /// stored on the profile.
    pub fn connection_settings(&self, secrets: &ProfileSecrets) -> ConnectionSettings {
        let mut settings = ConnectionSettings::new(&self.id, &self.bootstrap);
        settings.admin_timeout = Duration::from_millis(self.admin_timeout_ms.into());
        settings.delivery_timeout = Duration::from_millis(self.delivery_timeout_ms.into());
        let props = &mut settings.properties;
        props.push(("security.protocol".into(), self.security_protocol().into()));
        if let Some(sasl) = &self.sasl {
            props.push((
                "sasl.mechanism".into(),
                sasl.mechanism.librdkafka_name().into(),
            ));
            props.push(("sasl.username".into(), sasl.username.clone()));
            if let Some(password) = &secrets.sasl_password {
                props.push(("sasl.password".into(), password.clone()));
            }
        }
        if self.tls.enabled {
            if let Some(ca) = &self.tls.ca_path {
                props.push(("ssl.ca.location".into(), ca.clone()));
            }
            if let Some(cert) = &self.tls.cert_path {
                props.push(("ssl.certificate.location".into(), cert.clone()));
            }
            if let Some(key) = &self.tls.key_path {
                props.push(("ssl.key.location".into(), key.clone()));
            }
            if let Some(password) = &secrets.tls_key_password {
                props.push(("ssl.key.password".into(), password.clone()));
            }
            let algorithm = if self.tls.verify_hostname {
                "https"
            } else {
                "none"
            };
            props.push((
                "ssl.endpoint.identification.algorithm".into(),
                algorithm.into(),
            ));
        }
        settings
    }
}

/// Secret values for one profile, loaded from the OS credential store.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileSecrets {
    pub sasl_password: Option<String>,
    pub tls_key_password: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProfileFile {
    version: u32,
    #[serde(default)]
    active_id: Option<String>,
    profiles: Vec<Profile>,
}

#[derive(Debug, Default)]
struct Profiles {
    active_id: Option<String>,
    list: Vec<Profile>,
}

/// Profiles persisted to a JSON file, written atomically on every change.
pub struct ProfileStore {
    path: PathBuf,
    profiles: Mutex<Profiles>,
}

impl ProfileStore {
    pub fn load(path: impl Into<PathBuf>) -> AppResult<Self> {
        let path = path.into();
        let profiles = match std::fs::read(&path) {
            Ok(bytes) => {
                let file: ProfileFile = serde_json::from_slice(&bytes).map_err(|e| {
                    AppError::new(
                        ErrorKind::Config,
                        format!("{} is not valid: {e}", path.display()),
                    )
                })?;
                Profiles {
                    active_id: file
                        .active_id
                        .filter(|id| file.profiles.iter().any(|p| &p.id == id)),
                    list: file.profiles,
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Profiles::default(),
            Err(e) => {
                return Err(AppError::new(
                    ErrorKind::Config,
                    format!("could not read {}: {e}", path.display()),
                ))
            }
        };
        Ok(Self {
            path,
            profiles: Mutex::new(profiles),
        })
    }

    /// A store with no profiles that will write to `path` on first change.
    pub fn empty(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            profiles: Mutex::new(Profiles::default()),
        }
    }

    fn lock(&self) -> AppResult<std::sync::MutexGuard<'_, Profiles>> {
        self.profiles
            .lock()
            .map_err(|_| AppError::internal("profile store lock poisoned"))
    }

    pub fn list(&self) -> AppResult<Vec<Profile>> {
        Ok(self.lock()?.list.clone())
    }

    pub fn get(&self, id: &str) -> AppResult<Profile> {
        self.lock()?
            .list
            .iter()
            .find(|p| p.id == id)
            .cloned()
            .ok_or_else(|| AppError::not_found(format!("Connection {id}")))
    }

    pub fn create(&self, input: ProfileInput) -> AppResult<Profile> {
        input.validate()?;
        let mut profiles = self.lock()?;
        ensure_unique_name(&profiles.list, &input.name, None)?;
        let profile = input.into_profile(new_id());
        profiles.list.push(profile.clone());
        if profiles.active_id.is_none() {
            profiles.active_id = Some(profile.id.clone());
        }
        save(&self.path, &profiles)?;
        Ok(profile)
    }

    pub fn update(&self, id: &str, input: ProfileInput) -> AppResult<Profile> {
        input.validate()?;
        let mut profiles = self.lock()?;
        ensure_unique_name(&profiles.list, &input.name, Some(id))?;
        let slot = profiles
            .list
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| AppError::not_found(format!("Connection {id}")))?;
        *slot = input.into_profile(id.to_string());
        let updated = slot.clone();
        save(&self.path, &profiles)?;
        Ok(updated)
    }

    pub fn delete(&self, id: &str) -> AppResult<Profile> {
        let mut profiles = self.lock()?;
        let index = profiles
            .list
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| AppError::not_found(format!("Connection {id}")))?;
        let removed = profiles.list.remove(index);
        if profiles.active_id.as_deref() == Some(id) {
            profiles.active_id = profiles.list.first().map(|p| p.id.clone());
        }
        save(&self.path, &profiles)?;
        Ok(removed)
    }

    /// The selected profile, if any.
    pub fn active(&self) -> AppResult<Option<Profile>> {
        let profiles = self.lock()?;
        Ok(profiles
            .active_id
            .as_ref()
            .and_then(|id| profiles.list.iter().find(|p| &p.id == id))
            .cloned())
    }

    pub fn set_active(&self, id: &str) -> AppResult<Profile> {
        let mut profiles = self.lock()?;
        let profile = profiles
            .list
            .iter()
            .find(|p| p.id == id)
            .cloned()
            .ok_or_else(|| AppError::not_found(format!("Connection {id}")))?;
        profiles.active_id = Some(profile.id.clone());
        save(&self.path, &profiles)?;
        Ok(profile)
    }
}

fn ensure_unique_name(profiles: &[Profile], name: &str, except: Option<&str>) -> AppResult<()> {
    let taken = profiles
        .iter()
        .any(|p| p.name.eq_ignore_ascii_case(name.trim()) && Some(p.id.as_str()) != except);
    if taken {
        return Err(AppError::invalid_input(format!(
            "A connection named \"{}\" already exists.",
            name.trim()
        )));
    }
    Ok(())
}

fn save(path: &Path, profiles: &Profiles) -> AppResult<()> {
    let io = |e: std::io::Error| {
        AppError::new(
            ErrorKind::Config,
            format!("could not save {}: {e}", path.display()),
        )
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(io)?;
    }
    let file = ProfileFile {
        version: 1,
        active_id: profiles.active_id.clone(),
        profiles: profiles.list.clone(),
    };
    let json = serde_json::to_vec_pretty(&file)
        .map_err(|e| AppError::internal(format!("could not serialize profiles: {e}")))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(io)?;
    std::fs::rename(&tmp, path).map_err(io)?;
    Ok(())
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(name: &str, environment: Environment) -> ProfileInput {
        ProfileInput {
            name: name.into(),
            environment,
            bootstrap: "localhost:9092".into(),
            sasl: None,
            tls: TlsConfig::default(),
            read_only: false,
            admin_timeout_ms: None,
            delivery_timeout_ms: None,
        }
    }

    fn temp_store() -> (tempfile::TempDir, ProfileStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = ProfileStore::load(dir.path().join("profiles.json")).unwrap();
        (dir, store)
    }

    #[test]
    fn prod_profiles_are_always_read_only_at_rest() {
        let (_dir, store) = temp_store();
        let prod = store.create(input("prod", Environment::Prod)).unwrap();
        assert!(prod.read_only);
        let mut edit = input("prod", Environment::Prod);
        edit.read_only = false;
        assert!(store.update(&prod.id, edit).unwrap().read_only);

        let dev = store.create(input("dev", Environment::Dev)).unwrap();
        assert!(!dev.read_only);
    }

    #[test]
    fn profiles_survive_reload() {
        let (dir, store) = temp_store();
        let created = store.create(input("local", Environment::Local)).unwrap();
        drop(store);
        let reloaded = ProfileStore::load(dir.path().join("profiles.json")).unwrap();
        assert_eq!(reloaded.list().unwrap(), vec![created.clone()]);
        assert_eq!(reloaded.active().unwrap(), Some(created));
    }

    #[test]
    fn first_profile_becomes_active_and_delete_moves_selection() {
        let (_dir, store) = temp_store();
        let a = store.create(input("a", Environment::Local)).unwrap();
        let b = store.create(input("b", Environment::Dev)).unwrap();
        assert_eq!(store.active().unwrap().unwrap().id, a.id);
        store.set_active(&b.id).unwrap();
        store.delete(&b.id).unwrap();
        assert_eq!(store.active().unwrap().unwrap().id, a.id);
        store.delete(&a.id).unwrap();
        assert_eq!(store.active().unwrap(), None);
    }

    #[test]
    fn rejects_bad_input() {
        let (_dir, store) = temp_store();
        let mut bad = input("x", Environment::Local);
        bad.bootstrap = "localhost".into();
        assert_eq!(store.create(bad).unwrap_err().kind, ErrorKind::InvalidInput);
        store.create(input("dup", Environment::Local)).unwrap();
        assert_eq!(
            store
                .create(input("DUP", Environment::Dev))
                .unwrap_err()
                .kind,
            ErrorKind::InvalidInput
        );
    }

    #[test]
    fn security_protocol_and_properties() {
        let (_dir, store) = temp_store();
        let mut sasl = input("sasl", Environment::Dev);
        sasl.sasl = Some(SaslConfig {
            mechanism: SaslMechanism::ScramSha512,
            username: "alice".into(),
        });
        sasl.tls.enabled = true;
        let profile = store.create(sasl).unwrap();
        assert_eq!(profile.security_protocol(), "SASL_SSL");
        let settings = profile.connection_settings(&ProfileSecrets {
            sasl_password: Some("pw".into()),
            tls_key_password: None,
        });
        let props: std::collections::HashMap<_, _> = settings.properties.into_iter().collect();
        assert_eq!(props["sasl.mechanism"], "SCRAM-SHA-512");
        assert_eq!(props["sasl.password"], "pw");
        assert_eq!(props["ssl.endpoint.identification.algorithm"], "https");
    }
}
