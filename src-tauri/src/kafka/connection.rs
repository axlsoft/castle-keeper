//! "Test connection": connect once with throwaway clients and report what
//! the cluster looks like, or the most specific reason it failed.

use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

use rdkafka::{
    client::ClientContext,
    consumer::{BaseConsumer, Consumer, ConsumerContext},
    error::{KafkaError, RDKafkaErrorCode},
};
use serde::Serialize;
use specta::Type;

use super::{run_blocking, settings::ConnectionSettings};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BrokerInfo {
    pub id: i32,
    pub host: String,
    pub port: i32,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionReport {
    pub cluster_id: Option<String>,
    pub controller_id: Option<i32>,
    pub brokers: Vec<BrokerInfo>,
    pub topic_count: u32,
    /// Time to the first metadata response, including connect and auth.
    pub connect_ms: u32,
    /// A second metadata round trip on the established connection.
    pub round_trip_ms: u32,
}

/// Remembers the most specific error librdkafka reported while connecting.
#[derive(Default)]
struct CapturingContext {
    first_error: Mutex<Option<(RDKafkaErrorCode, String)>>,
}

impl ClientContext for CapturingContext {
    fn error(&self, error: KafkaError, reason: &str) {
        tracing::debug!(%error, reason, "connection test error");
        let Some(code) = error.rdkafka_error_code() else {
            return;
        };
        if let Ok(mut slot) = self.first_error.lock() {
            // "All brokers down" is a consequence; keep the underlying cause.
            let replace = match slot.as_ref() {
                None => true,
                Some((existing, _)) => *existing == RDKafkaErrorCode::AllBrokersDown,
            };
            if replace {
                *slot = Some((code, reason.to_string()));
            }
        }
    }
}

impl ConsumerContext for CapturingContext {}

pub async fn test_connection(settings: ConnectionSettings) -> AppResult<ConnectionReport> {
    let timeout = settings.admin_timeout;
    run_blocking("connecting", timeout, move || probe(&settings, timeout)).await
}

fn probe(settings: &ConnectionSettings, timeout: Duration) -> AppResult<ConnectionReport> {
    let consumer: BaseConsumer<CapturingContext> = settings
        .client_config()
        .create_with_context(CapturingContext::default())?;

    let started = Instant::now();
    let metadata = consumer.fetch_metadata(None, timeout);
    let connect = started.elapsed();
    // Serve queued events so the error callback has run before we look.
    let _ = consumer.poll(Duration::from_millis(100));

    let metadata = match metadata {
        Ok(metadata) => metadata,
        Err(error) => return Err(explain(error, &consumer)),
    };

    let started = Instant::now();
    consumer.fetch_metadata(None, timeout)?;
    let round_trip = started.elapsed();

    let cluster_id = consumer.client().fetch_cluster_id(timeout);
    let controller_id = controller_id(&consumer, timeout);

    Ok(ConnectionReport {
        cluster_id,
        controller_id,
        brokers: metadata
            .brokers()
            .iter()
            .map(|b| BrokerInfo {
                id: b.id(),
                host: b.host().to_string(),
                port: b.port(),
            })
            .collect(),
        topic_count: u32::try_from(metadata.topics().len()).unwrap_or(u32::MAX),
        connect_ms: millis(connect),
        round_trip_ms: millis(round_trip),
    })
}

fn controller_id(consumer: &BaseConsumer<CapturingContext>, timeout: Duration) -> Option<i32> {
    let timeout_ms = i32::try_from(timeout.as_millis()).unwrap_or(i32::MAX);
    // SAFETY: the client pointer is valid for the lifetime of `consumer`, and
    // rd_kafka_controllerid only reads cached metadata (or waits for it).
    let id = unsafe {
        rdkafka::bindings::rd_kafka_controllerid(consumer.client().native_ptr(), timeout_ms)
    };
    (id >= 0).then_some(id)
}

/// Prefers the specific cause seen in librdkafka's error callback (DNS,
/// refused, TLS, auth) over the generic metadata error.
fn explain(error: KafkaError, consumer: &BaseConsumer<CapturingContext>) -> AppError {
    let captured = consumer
        .context()
        .first_error
        .lock()
        .ok()
        .and_then(|slot| slot.clone());
    match captured {
        Some((code, reason)) => {
            let mut specific = AppError::from(KafkaError::Global(code));
            specific.message = reason;
            specific
        }
        None => AppError::from(error),
    }
}

fn millis(duration: Duration) -> u32 {
    u32::try_from(duration.as_millis()).unwrap_or(u32::MAX)
}
