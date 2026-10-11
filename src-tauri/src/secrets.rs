//! Connection secrets in the OS credential store (macOS Keychain, Windows
//! Credential Manager, Secret Service on Linux). Profiles never hold them.

use std::{collections::HashMap, sync::Mutex};

use crate::{
    error::{AppError, AppResult, ErrorKind},
    profiles::ProfileSecrets,
};

const SERVICE: &str = "castle-keeper";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecretField {
    SaslPassword,
    TlsKeyPassword,
}

impl SecretField {
    const ALL: [SecretField; 2] = [SecretField::SaslPassword, SecretField::TlsKeyPassword];

    fn account(self, profile_id: &str) -> String {
        let field = match self {
            SecretField::SaslPassword => "sasl-password",
            SecretField::TlsKeyPassword => "tls-key-password",
        };
        format!("{profile_id}/{field}")
    }
}

pub trait SecretStore: Send + Sync {
    fn get(&self, profile_id: &str, field: SecretField) -> AppResult<Option<String>>;
    fn set(&self, profile_id: &str, field: SecretField, value: &str) -> AppResult<()>;
    fn delete(&self, profile_id: &str, field: SecretField) -> AppResult<()>;

    fn load(&self, profile_id: &str) -> AppResult<ProfileSecrets> {
        Ok(ProfileSecrets {
            sasl_password: self.get(profile_id, SecretField::SaslPassword)?,
            tls_key_password: self.get(profile_id, SecretField::TlsKeyPassword)?,
        })
    }

    fn delete_all(&self, profile_id: &str) -> AppResult<()> {
        for field in SecretField::ALL {
            self.delete(profile_id, field)?;
        }
        Ok(())
    }
}

fn secret_error(error: keyring::Error) -> AppError {
    AppError::new(
        ErrorKind::Secret,
        format!("credential store error: {error}"),
    )
    .with_hint("Make sure the system keychain / credential manager is unlocked and available.")
}

/// The platform credential store.
pub struct KeyringStore;

impl KeyringStore {
    fn entry(profile_id: &str, field: SecretField) -> AppResult<keyring::Entry> {
        keyring::Entry::new(SERVICE, &field.account(profile_id)).map_err(secret_error)
    }
}

impl SecretStore for KeyringStore {
    fn get(&self, profile_id: &str, field: SecretField) -> AppResult<Option<String>> {
        match Self::entry(profile_id, field)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(secret_error(error)),
        }
    }

    fn set(&self, profile_id: &str, field: SecretField, value: &str) -> AppResult<()> {
        Self::entry(profile_id, field)?
            .set_password(value)
            .map_err(secret_error)
    }

    fn delete(&self, profile_id: &str, field: SecretField) -> AppResult<()> {
        match Self::entry(profile_id, field)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(secret_error(error)),
        }
    }
}

/// In-memory store for tests and environments without a credential store.
#[derive(Default)]
pub struct MemoryStore {
    values: Mutex<HashMap<String, String>>,
}

impl MemoryStore {
    fn lock(&self) -> AppResult<std::sync::MutexGuard<'_, HashMap<String, String>>> {
        self.values
            .lock()
            .map_err(|_| AppError::internal("secret store lock poisoned"))
    }
}

impl SecretStore for MemoryStore {
    fn get(&self, profile_id: &str, field: SecretField) -> AppResult<Option<String>> {
        Ok(self.lock()?.get(&field.account(profile_id)).cloned())
    }

    fn set(&self, profile_id: &str, field: SecretField, value: &str) -> AppResult<()> {
        self.lock()?
            .insert(field.account(profile_id), value.to_string());
        Ok(())
    }

    fn delete(&self, profile_id: &str, field: SecretField) -> AppResult<()> {
        self.lock()?.remove(&field.account(profile_id));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Touches the real OS credential store; run with `--ignored`.
    #[test]
    #[ignore]
    fn keyring_store_round_trip() {
        let store = KeyringStore;
        let id = format!("test-{}", uuid::Uuid::new_v4());
        store.set(&id, SecretField::SaslPassword, "s3cret").unwrap();
        assert_eq!(
            store
                .get(&id, SecretField::SaslPassword)
                .unwrap()
                .as_deref(),
            Some("s3cret")
        );
        store.delete_all(&id).unwrap();
        assert_eq!(store.get(&id, SecretField::SaslPassword).unwrap(), None);
    }

    #[test]
    fn memory_store_round_trip() {
        let store = MemoryStore::default();
        store
            .set("p1", SecretField::SaslPassword, "s3cret")
            .unwrap();
        assert_eq!(
            store.load("p1").unwrap().sasl_password.as_deref(),
            Some("s3cret")
        );
        assert_eq!(store.load("p2").unwrap(), ProfileSecrets::default());
        store.delete_all("p1").unwrap();
        assert_eq!(store.load("p1").unwrap(), ProfileSecrets::default());
    }
}
