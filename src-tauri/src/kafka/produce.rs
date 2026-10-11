use rdkafka::{
    message::{Header, OwnedHeaders},
    producer::FutureRecord,
    util::Timeout,
};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::manager::ClusterClients;
use crate::{
    error::{AppError, AppResult},
    guard::WriteTicket,
};

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProduceRequest {
    pub topic: String,
    #[serde(default)]
    pub partition: Option<i32>,
    #[serde(default)]
    pub key: Option<String>,
    pub value: String,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProduceOutcome {
    /// True when the request was validated and previewed but not sent.
    pub dry_run: bool,
    pub topic: String,
    pub partition: Option<i32>,
    pub offset: Option<i64>,
    pub key_bytes: u32,
    pub value_bytes: u32,
    pub header_count: u32,
}

impl ProduceRequest {
    pub fn validate(&self) -> AppResult<()> {
        if self.topic.trim().is_empty() {
            return Err(AppError::invalid_input("Topic is required"));
        }
        Ok(())
    }
}

/// Produces one message. Only the write guard can issue the ticket this
/// requires; the outcome is recorded in the action log when it completes.
pub async fn produce(
    clients: &ClusterClients,
    request: &ProduceRequest,
    ticket: WriteTicket,
) -> AppResult<ProduceOutcome> {
    if ticket.target() != request.topic {
        let error = AppError::internal("write ticket was issued for a different topic");
        ticket.complete(&Err::<(), _>(error.clone()));
        return Err(error);
    }
    let result = send(clients, request, ticket.is_dry_run()).await;
    match &result {
        Ok(outcome) if !outcome.dry_run => ticket.complete_with_detail(
            &result,
            format!(
                "partition {} offset {}",
                outcome.partition.unwrap_or(-1),
                outcome.offset.unwrap_or(-1)
            ),
        ),
        _ => ticket.complete(&result),
    }
    result
}

async fn send(
    clients: &ClusterClients,
    request: &ProduceRequest,
    dry_run: bool,
) -> AppResult<ProduceOutcome> {
    request.validate()?;
    let mut outcome = ProduceOutcome {
        dry_run,
        topic: request.topic.clone(),
        partition: request.partition,
        offset: None,
        key_bytes: len_u32(request.key.as_deref().unwrap_or_default()),
        value_bytes: len_u32(&request.value),
        header_count: u32::try_from(request.headers.len()).unwrap_or(u32::MAX),
    };
    if dry_run {
        return Ok(outcome);
    }

    let producer = clients.producer()?;
    let timeout = clients.settings().delivery_timeout;

    let mut headers = OwnedHeaders::new();
    for (key, value) in &request.headers {
        headers = headers.insert(Header {
            key,
            value: Some(value.as_bytes()),
        });
    }
    let mut record = FutureRecord::to(&request.topic)
        .payload(request.value.as_bytes())
        .headers(headers);
    if let Some(key) = &request.key {
        record = record.key(key.as_bytes());
    }
    if let Some(partition) = request.partition {
        record = record.partition(partition);
    }

    let delivery = tokio::time::timeout(
        timeout + std::time::Duration::from_secs(1),
        producer.send(record, Timeout::After(timeout)),
    )
    .await
    .map_err(|_| AppError::timeout("delivering the message", timeout))?
    .map_err(|(error, _message)| AppError::from(error))?;

    outcome.partition = Some(delivery.partition);
    outcome.offset = Some(delivery.offset);
    Ok(outcome)
}

fn len_u32(text: &str) -> u32 {
    u32::try_from(text.len()).unwrap_or(u32::MAX)
}
