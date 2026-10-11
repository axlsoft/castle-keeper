use std::time::Duration;

use rdkafka::{
    producer::{FutureProducer, FutureRecord},
    ClientConfig,
};

use crate::error::AppResult;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PublishMessage {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PublisherResult {
    pub partition: i32,
    pub offset: i64,
}

#[tauri::command]
pub async fn send_message(message: PublishMessage) -> AppResult<PublisherResult> {
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", "localhost:9092")
        // Fail a stuck delivery in seconds rather than librdkafka's 5 minute default.
        .set("message.timeout.ms", "15000")
        .create()?;

    let topic = "my_topic";

    let delivery = producer
        .send(
            FutureRecord::to(topic)
                .payload(&message.value)
                .key(&message.key),
            Duration::from_secs(0),
        )
        .await
        .map_err(|(error, _message)| error)?;

    Ok(PublisherResult {
        partition: delivery.partition,
        offset: delivery.offset,
    })
}
