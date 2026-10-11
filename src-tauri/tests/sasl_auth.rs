//! SASL/PLAIN and SASL/SCRAM against the `kafka-sasl` compose service.
//!
//! These prove librdkafka's built-in SASL works without Cyrus SASL.
//! Skipped unless `CK_IT_SASL_BOOTSTRAP` is set, e.g.:
//!
//! ```sh
//! docker compose --profile sasl up -d --wait
//! CK_IT_SASL_BOOTSTRAP=127.0.0.1:9094 cargo test --test sasl_auth
//! ```

use std::time::Duration;

use rdkafka::{
    consumer::{BaseConsumer, Consumer},
    error::KafkaResult,
    ClientConfig,
};

fn bootstrap() -> Option<String> {
    let value = std::env::var("CK_IT_SASL_BOOTSTRAP").ok();
    if value.is_none() {
        eprintln!("CK_IT_SASL_BOOTSTRAP not set; skipping");
    }
    value
}

fn fetch_metadata(
    bootstrap: &str,
    mechanism: &str,
    user: &str,
    password: &str,
) -> KafkaResult<usize> {
    let consumer: BaseConsumer = ClientConfig::new()
        .set("bootstrap.servers", bootstrap)
        .set("security.protocol", "SASL_PLAINTEXT")
        .set("sasl.mechanism", mechanism)
        .set("sasl.username", user)
        .set("sasl.password", password)
        .create()?;
    let metadata = consumer.fetch_metadata(None, Duration::from_secs(10))?;
    Ok(metadata.brokers().len())
}

#[test]
fn builtin_sasl_mechanisms_authenticate() {
    let Some(bootstrap) = bootstrap() else { return };
    for mechanism in ["PLAIN", "SCRAM-SHA-256", "SCRAM-SHA-512"] {
        let brokers = fetch_metadata(&bootstrap, mechanism, "alice", "alice-secret")
            .unwrap_or_else(|e| panic!("{mechanism} failed: {e}"));
        assert_eq!(brokers, 1, "{mechanism}");
    }
}

#[test]
fn wrong_password_is_rejected() {
    let Some(bootstrap) = bootstrap() else { return };
    for mechanism in ["PLAIN", "SCRAM-SHA-256"] {
        let result = fetch_metadata(&bootstrap, mechanism, "alice", "wrong");
        assert!(result.is_err(), "{mechanism} accepted a wrong password");
    }
}
