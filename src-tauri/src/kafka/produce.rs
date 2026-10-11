use rdkafka::{
    message::{Header, OwnedHeaders},
    producer::FutureRecord,
    util::Timeout,
};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::manager::ClusterClients;
use crate::error::{AppError, AppResult};

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
pub struct ProduceResult {
    pub partition: i32,
    pub offset: i64,
}

impl ProduceRequest {
    pub fn validate(&self) -> AppResult<()> {
        if self.topic.trim().is_empty() {
            return Err(AppError::invalid_input("Topic is required"));
        }
        Ok(())
    }
}

pub async fn produce(
    clients: &ClusterClients,
    request: &ProduceRequest,
) -> AppResult<ProduceResult> {
    request.validate()?;
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

    Ok(ProduceResult {
        partition: delivery.partition,
        offset: delivery.offset,
    })
}
