//! Read-only message browsing. A short-lived consumer is positioned with
//! `assign()` at explicit offsets; it never subscribes (no JoinGroup) and
//! never commits (no OffsetCommit), so browsing can't create, join or move a
//! consumer group. librdkafka still requires a `group.id` for `assign()`, so
//! each browse uses a unique throwaway id that the broker never records.

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use rdkafka::{
    Offset, Timestamp, TopicPartitionList,
    consumer::{BaseConsumer, Consumer},
    error::KafkaError,
    message::{Headers, Message},
    types::RDKafkaErrorCode,
};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::{manager::ClusterClients, offsets, run_blocking, settings::ConnectionSettings};
use crate::error::{AppError, AppResult};

/// Messages larger than this are cut, with `truncated` set.
pub const MAX_PAYLOAD_BYTES: usize = 1024 * 1024;
const BATCH_INTERVAL: Duration = Duration::from_millis(50);
const BATCH_SIZE: usize = 500;

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum StartPosition {
    Earliest,
    /// The last `count` messages of each partition.
    Latest {
        count: u32,
    },
    Offset {
        offset: i64,
    },
    /// Unix time in milliseconds.
    Timestamp {
        timestamp: i64,
    },
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BrowseRequest {
    pub topic: String,
    /// Partitions to read; all when empty.
    #[serde(default)]
    pub partitions: Vec<i32>,
    pub start: StartPosition,
    /// Stop after this many messages in total.
    pub limit: u32,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BrowsedHeader {
    pub key: String,
    /// Base64; null for a header without a value.
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BrowsedMessage {
    pub partition: i32,
    pub offset: i64,
    /// Milliseconds since the epoch, when the message has one.
    pub timestamp: Option<i64>,
    /// "create", "logAppend" or null.
    pub timestamp_type: Option<String>,
    /// Base64; null for a null key.
    pub key: Option<String>,
    /// Base64; null for a tombstone.
    pub value: Option<String>,
    pub value_size: u32,
    pub truncated: bool,
    pub headers: Vec<BrowsedHeader>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PartitionRange {
    pub partition: i32,
    pub start: i64,
    /// High watermark when the browse started; reading stops here.
    pub end: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum StopReason {
    /// Every partition reached its end offset.
    Caught,
    Limit,
    Cancelled,
    /// No new messages arrived for the admin timeout.
    Idle,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum BrowseEvent {
    Started {
        ranges: Vec<PartitionRange>,
    },
    Batch {
        messages: Vec<BrowsedMessage>,
    },
    Done {
        reason: StopReason,
        count: u32,
        elapsed_ms: u32,
    },
}

fn encode(bytes: &[u8]) -> (String, bool) {
    let truncated = bytes.len() > MAX_PAYLOAD_BYTES;
    let slice = if truncated {
        &bytes[..MAX_PAYLOAD_BYTES]
    } else {
        bytes
    };
    (BASE64.encode(slice), truncated)
}

fn to_browsed<M: Message>(message: &M) -> BrowsedMessage {
    let (timestamp, timestamp_type) = match message.timestamp() {
        Timestamp::NotAvailable => (None, None),
        Timestamp::CreateTime(t) => (Some(t), Some("create".to_string())),
        Timestamp::LogAppendTime(t) => (Some(t), Some("logAppend".to_string())),
    };
    let payload = message.payload();
    let (value, truncated) = match payload {
        Some(bytes) => {
            let (encoded, truncated) = encode(bytes);
            (Some(encoded), truncated)
        }
        None => (None, false),
    };
    let headers = message
        .headers()
        .map(|headers| {
            headers
                .iter()
                .map(|h| BrowsedHeader {
                    key: h.key.to_string(),
                    value: h.value.map(|v| BASE64.encode(v)),
                })
                .collect()
        })
        .unwrap_or_default();
    BrowsedMessage {
        partition: message.partition(),
        offset: message.offset(),
        timestamp,
        timestamp_type,
        key: message.key().map(|k| BASE64.encode(k)),
        value,
        value_size: u32::try_from(payload.map_or(0, <[u8]>::len)).unwrap_or(u32::MAX),
        truncated,
        headers,
    }
}

/// Group id prefix for browse consumers; such ids never reach the broker's
/// group coordinator as a member or a commit.
pub const BROWSE_GROUP_PREFIX: &str = "castle-keeper-browse-";

fn browse_config(settings: &ConnectionSettings) -> rdkafka::ClientConfig {
    let mut config = settings.client_config();
    config
        .set("client.id", "castle-keeper-browser")
        // Overrides any group id from the connection's extra properties.
        .set(
            "group.id",
            format!("{BROWSE_GROUP_PREFIX}{}", uuid::Uuid::new_v4()),
        )
        .set("enable.auto.commit", "false")
        .set("enable.auto.offset.store", "false")
        .set("enable.partition.eof", "true")
        .set("auto.offset.reset", "earliest");
    config
}

/// The consumer used for browsing: commits disabled, assign-only.
pub fn browse_consumer(settings: &ConnectionSettings) -> AppResult<BaseConsumer> {
    Ok(browse_config(settings).create()?)
}

/// Works out where each partition starts and ends.
fn plan(
    clients: &ClusterClients,
    request: &BrowseRequest,
    timeout: Duration,
) -> AppResult<Vec<PartitionRange>> {
    let metadata = clients
        .metadata_client()
        .fetch_metadata(Some(&request.topic), timeout)?;
    let topic = metadata
        .topics()
        .iter()
        .find(|t| t.name() == request.topic)
        .ok_or_else(|| AppError::not_found(format!("Topic {}", request.topic)))?;
    if let Some(error) = topic.error() {
        return Err(KafkaError::MetadataFetch(error.into()).into());
    }
    let available: Vec<i32> = topic.partitions().iter().map(|p| p.id()).collect();
    let partitions: Vec<i32> = if request.partitions.is_empty() {
        available.clone()
    } else {
        for p in &request.partitions {
            if !available.contains(p) {
                return Err(AppError::invalid_input(format!(
                    "{} has no partition {p} (it has {}).",
                    request.topic,
                    available.len()
                )));
            }
        }
        request.partitions.clone()
    };
    let keys: Vec<_> = partitions
        .iter()
        .map(|p| (request.topic.clone(), *p))
        .collect();
    let marks = offsets::watermarks(clients.metadata_client(), &keys, timeout)?;

    let by_time: HashMap<i32, i64> = match request.start {
        StartPosition::Timestamp { timestamp } => {
            let mut tpl = TopicPartitionList::new();
            for p in &partitions {
                tpl.add_partition_offset(&request.topic, *p, Offset::Offset(timestamp))?;
            }
            clients
                .metadata_client()
                .offsets_for_times(tpl, timeout)?
                .elements()
                .iter()
                .map(|e| {
                    let offset = match e.offset() {
                        Offset::Offset(o) => o,
                        // No message at or after the timestamp: start at the end.
                        _ => i64::MAX,
                    };
                    (e.partition(), offset)
                })
                .collect()
        }
        _ => HashMap::new(),
    };

    Ok(partitions
        .iter()
        .map(|p| {
            let (low, high) = marks
                .get(&(request.topic.clone(), *p))
                .copied()
                .unwrap_or((0, 0));
            let start = match request.start {
                StartPosition::Earliest => low,
                StartPosition::Latest { count } => (high - i64::from(count)).max(low),
                StartPosition::Offset { offset } => offset.clamp(low, high),
                StartPosition::Timestamp { .. } => {
                    by_time.get(p).copied().unwrap_or(high).clamp(low, high)
                }
            };
            PartitionRange {
                partition: *p,
                start,
                end: high,
            }
        })
        .collect())
}

fn is_transient(error: &KafkaError) -> bool {
    matches!(
        error.rdkafka_error_code(),
        Some(
            RDKafkaErrorCode::BrokerTransportFailure
                | RDKafkaErrorCode::AllBrokersDown
                | RDKafkaErrorCode::OperationTimedOut
                | RDKafkaErrorCode::RequestTimedOut
                | RDKafkaErrorCode::NotLeaderForPartition
                | RDKafkaErrorCode::LeaderNotAvailable
        )
    )
}

/// Reads messages and hands them to `emit` in batches. Blocking; run it on
/// the blocking pool.
pub fn browse(
    clients: &ClusterClients,
    request: &BrowseRequest,
    cancel: &AtomicBool,
    emit: &mut dyn FnMut(BrowseEvent) -> AppResult<()>,
) -> AppResult<()> {
    let started = Instant::now();
    let settings = clients.settings();
    let timeout = settings.admin_timeout;
    if request.limit == 0 {
        return Err(AppError::invalid_input("Limit must be at least 1."));
    }
    let ranges = plan(clients, request, timeout)?;
    emit(BrowseEvent::Started {
        ranges: ranges.clone(),
    })?;

    let mut remaining: HashMap<i32, i64> = ranges
        .iter()
        .filter(|r| r.start < r.end)
        .map(|r| (r.partition, r.end))
        .collect();
    let mut count: u32 = 0;
    let finish =
        |reason: StopReason, count: u32, emit: &mut dyn FnMut(BrowseEvent) -> AppResult<()>| {
            emit(BrowseEvent::Done {
                reason,
                count,
                elapsed_ms: u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX),
            })
        };
    if remaining.is_empty() {
        return finish(StopReason::Caught, 0, emit);
    }

    let consumer = browse_consumer(settings)?;
    let mut tpl = TopicPartitionList::new();
    for range in ranges
        .iter()
        .filter(|r| remaining.contains_key(&r.partition))
    {
        tpl.add_partition_offset(&request.topic, range.partition, Offset::Offset(range.start))?;
    }
    consumer.assign(&tpl)?;

    let mut batch = Vec::new();
    let mut last_flush = Instant::now();
    let mut last_message = Instant::now();
    let mut last_error: Option<KafkaError> = None;
    let reason = loop {
        if cancel.load(Ordering::Relaxed) {
            break StopReason::Cancelled;
        }
        if remaining.is_empty() {
            break StopReason::Caught;
        }
        if count >= request.limit {
            break StopReason::Limit;
        }
        if last_message.elapsed() > timeout {
            if count == 0
                && let Some(error) = last_error.take()
            {
                return Err(error.into());
            }
            break StopReason::Idle;
        }
        match consumer.poll(Duration::from_millis(50)) {
            Some(Ok(message)) => {
                last_message = Instant::now();
                let partition = message.partition();
                let Some(&end) = remaining.get(&partition) else {
                    continue;
                };
                if message.offset() >= end {
                    remaining.remove(&partition);
                    continue;
                }
                batch.push(to_browsed(&message));
                count += 1;
                if message.offset() + 1 >= end {
                    remaining.remove(&partition);
                }
            }
            Some(Err(KafkaError::PartitionEOF(partition))) => {
                remaining.remove(&partition);
            }
            Some(Err(error))
                if error.rdkafka_error_code() == Some(RDKafkaErrorCode::OffsetOutOfRange) =>
            {
                return Err(AppError::invalid_input(
                    "The start offset is no longer available (retention removed it). Start from the earliest offset instead.",
                ));
            }
            Some(Err(error)) if is_transient(&error) => {
                // Connection hiccups (e.g. an IPv6 attempt before IPv4) are
                // retried by librdkafka; only give up if nothing arrives.
                tracing::debug!(%error, "transient error while browsing");
                last_error = Some(error);
            }
            Some(Err(error)) => return Err(error.into()),
            None => {}
        }
        if batch.len() >= BATCH_SIZE
            || (!batch.is_empty() && last_flush.elapsed() >= BATCH_INTERVAL)
        {
            emit(BrowseEvent::Batch {
                messages: std::mem::take(&mut batch),
            })?;
            last_flush = Instant::now();
        }
    };
    if !batch.is_empty() {
        emit(BrowseEvent::Batch { messages: batch })?;
    }
    finish(reason, count, emit)
}

/// Async wrapper: runs the browse on the blocking pool.
pub async fn browse_async(
    clients: Arc<ClusterClients>,
    request: BrowseRequest,
    cancel: Arc<AtomicBool>,
    mut emit: impl FnMut(BrowseEvent) -> AppResult<()> + Send + 'static,
) -> AppResult<()> {
    // Generous outer bound: the loop itself stops on idle, limit or cancel.
    let bound = clients.settings().admin_timeout * 60;
    run_blocking("browsing messages", bound, move || {
        browse(&clients, &request, &cancel, &mut emit)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_payloads_are_truncated() {
        let (encoded, truncated) = encode(&vec![b'a'; MAX_PAYLOAD_BYTES + 10]);
        assert!(truncated);
        assert_eq!(BASE64.decode(encoded).unwrap().len(), MAX_PAYLOAD_BYTES);
        assert_eq!(encode(b"hi"), (BASE64.encode(b"hi"), false));
    }

    #[test]
    fn browse_consumer_has_no_group() {
        let mut settings = ConnectionSettings::new("x", "127.0.0.1:1");
        settings
            .properties
            .push(("group.id".into(), "leaked".into()));
        let mut config = settings.client_config();
        config.remove("group.id");
        assert_eq!(config.get("group.id"), None);
    }
}
