//! "Test connection" reports and error classification.
//!
//! Live checks are skipped unless `CK_IT_BOOTSTRAP` / `CK_IT_SASL_BOOTSTRAP`
//! are set; refused and DNS failures need no broker.

use std::time::Duration;

use castle_keeper_lib::{
    error::ErrorKind,
    kafka::{connection::test_connection, settings::ConnectionSettings},
};

fn settings(bootstrap: &str) -> ConnectionSettings {
    let mut settings = ConnectionSettings::new("test", bootstrap);
    settings.admin_timeout = Duration::from_secs(3);
    settings
}

#[tokio::test]
async fn refused_connection_is_reported_as_unreachable() {
    let error = test_connection(settings("127.0.0.1:1")).await.unwrap_err();
    assert_eq!(error.kind, ErrorKind::Unreachable, "{error:?}");
    assert!(error.message.contains("refused"), "{}", error.message);
    assert!(error.hint.is_some());
}

#[tokio::test]
async fn unknown_host_is_reported_as_unreachable() {
    // `.invalid` is reserved and never resolves.
    let error = test_connection(settings("no-such-broker.invalid:9092"))
        .await
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Unreachable, "{error:?}");
}

#[tokio::test]
async fn live_cluster_report() {
    let Ok(bootstrap) = std::env::var("CK_IT_BOOTSTRAP") else {
        eprintln!("CK_IT_BOOTSTRAP not set; skipping");
        return;
    };
    let report = test_connection(settings(&bootstrap)).await.unwrap();
    assert_eq!(report.brokers.len(), 1);
    assert!(report.cluster_id.is_some());
    assert_eq!(report.controller_id, Some(1));
    assert!(report.round_trip_ms < 1_000);
}

#[tokio::test]
async fn bad_sasl_password_is_reported_as_auth() {
    let Ok(bootstrap) = std::env::var("CK_IT_SASL_BOOTSTRAP") else {
        eprintln!("CK_IT_SASL_BOOTSTRAP not set; skipping");
        return;
    };
    let mut settings = settings(&bootstrap);
    settings.properties = vec![
        ("security.protocol".into(), "SASL_PLAINTEXT".into()),
        ("sasl.mechanism".into(), "SCRAM-SHA-256".into()),
        ("sasl.username".into(), "alice".into()),
        ("sasl.password".into(), "wrong".into()),
    ];
    let error = test_connection(settings).await.unwrap_err();
    assert_eq!(error.kind, ErrorKind::Auth, "{error:?}");
}
