use std::time::Duration;

use rdkafka::ClientConfig;

pub const DEFAULT_ADMIN_TIMEOUT: Duration = Duration::from_secs(10);
pub const DEFAULT_DELIVERY_TIMEOUT: Duration = Duration::from_secs(15);

/// Everything needed to build clients for one cluster connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionSettings {
    /// Stable key for the client cache (the profile id).
    pub id: String,
    pub bootstrap: String,
    /// Additional librdkafka properties (security protocol, SASL, TLS, ...).
    pub properties: Vec<(String, String)>,
    pub admin_timeout: Duration,
    pub delivery_timeout: Duration,
}

impl ConnectionSettings {
    pub fn new(id: impl Into<String>, bootstrap: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            bootstrap: bootstrap.into(),
            properties: Vec::new(),
            admin_timeout: DEFAULT_ADMIN_TIMEOUT,
            delivery_timeout: DEFAULT_DELIVERY_TIMEOUT,
        }
    }

    /// The implicit connection to the local development broker.
    pub fn local() -> Self {
        Self::new("local", "127.0.0.1:9092")
    }

    pub fn client_config(&self) -> ClientConfig {
        let mut config = ClientConfig::new();
        config
            .set("bootstrap.servers", &self.bootstrap)
            .set("client.id", "castle-keeper")
            .set(
                "socket.timeout.ms",
                self.admin_timeout.as_millis().to_string(),
            )
            // Surface connection failures quickly instead of retrying silently.
            .set("reconnect.backoff.max.ms", "2000");
        for (key, value) in &self.properties {
            config.set(key, value);
        }
        config
    }
}
