//! Helpers shared by the integration tests.
//!
//! Tests that need a broker read its address from the environment and
//! return early (passing) when it is unset, so `cargo test` works without
//! Docker. `scripts/test-integration.sh` starts the brokers and sets them.
#![allow(dead_code)]

use std::{sync::Arc, time::Duration};

use castle_keeper_lib::{
    audit::{ActionLog, WriteAction},
    guard::{SessionLocks, WriteGuard, WriteRequest, WriteTicket},
    kafka::settings::ConnectionSettings,
    profiles::{Environment, Profile, TlsConfig},
};

/// A local port nothing listens on, so connections are refused at once.
pub const CLOSED_PORT: &str = "127.0.0.1:1";

fn env_or_skip(name: &str) -> Option<String> {
    let value = std::env::var(name).ok();
    if value.is_none() {
        eprintln!("{name} not set; skipping");
    }
    value
}

/// Plaintext broker from `docker compose up` (CK_IT_BOOTSTRAP).
pub fn live_bootstrap() -> Option<String> {
    env_or_skip("CK_IT_BOOTSTRAP")
}

/// SASL broker from `docker compose --profile sasl up` (CK_IT_SASL_BOOTSTRAP).
pub fn sasl_bootstrap() -> Option<String> {
    env_or_skip("CK_IT_SASL_BOOTSTRAP")
}

/// Settings for a broker that refuses connections, with short timeouts.
pub fn down_settings(id: &str) -> ConnectionSettings {
    let mut settings = ConnectionSettings::new(id, CLOSED_PORT);
    settings.admin_timeout = Duration::from_secs(2);
    settings.delivery_timeout = Duration::from_secs(2);
    settings
}

pub fn profile(environment: Environment, read_only: bool) -> Profile {
    Profile {
        id: "it".into(),
        name: "it-profile".into(),
        environment,
        bootstrap: CLOSED_PORT.into(),
        sasl: None,
        tls: TlsConfig::default(),
        read_only,
        admin_timeout_ms: 2_000,
        delivery_timeout_ms: 2_000,
    }
}

pub fn guard(dir: &tempfile::TempDir) -> WriteGuard {
    WriteGuard::new(
        Arc::new(ActionLog::new(dir.path().join("actions.jsonl"))),
        Arc::new(SessionLocks::default()),
    )
}

/// A produce ticket for a writable local profile.
pub fn ticket(guard: &WriteGuard, topic: &str, dry_run: bool) -> WriteTicket {
    guard
        .authorize(
            &profile(Environment::Local, false),
            WriteRequest {
                action: WriteAction::Produce,
                target: topic.into(),
                dry_run,
                confirmation: None,
            },
        )
        .expect("local profile is writable")
}
