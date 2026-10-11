//! A SASL profile whose password lives only in the secret store connects.
//! Skipped unless `CK_IT_SASL_BOOTSTRAP` is set.

use std::sync::Arc;

use castle_keeper_lib::{
    kafka::connection::test_connection,
    profiles::{Environment, ProfileInput, ProfileStore, SaslConfig, SaslMechanism, TlsConfig},
    secrets::MemoryStore,
    state::AppState,
};

#[tokio::test]
async fn scram_profile_connects_with_stored_password() {
    let Ok(bootstrap) = std::env::var("CK_IT_SASL_BOOTSTRAP") else {
        eprintln!("CK_IT_SASL_BOOTSTRAP not set; skipping");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new(
        ProfileStore::load(dir.path().join("profiles.json")).unwrap(),
        Arc::new(MemoryStore::default()),
    );
    state
        .create_profile(ProfileInput {
            name: "sasl".into(),
            environment: Environment::Dev,
            bootstrap,
            sasl: Some(SaslConfig {
                mechanism: SaslMechanism::ScramSha512,
                username: "alice".into(),
            }),
            tls: TlsConfig::default(),
            read_only: false,
            admin_timeout_ms: Some(5_000),
            delivery_timeout_ms: None,
            sasl_password: Some("alice-secret".into()),
            tls_key_password: None,
        })
        .unwrap();

    let (_, settings) = state.active_connection().unwrap();
    let report = test_connection(settings).await.unwrap();
    assert_eq!(report.brokers.len(), 1);

    let on_disk = std::fs::read_to_string(dir.path().join("profiles.json")).unwrap();
    assert!(!on_disk.contains("alice-secret"));
}
