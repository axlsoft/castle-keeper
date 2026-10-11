use std::time::Duration;

use rdkafka::{
    consumer::{BaseConsumer, Consumer},
    ClientConfig,
};

use crate::error::AppResult;

#[tauri::command]
pub async fn get_topics() -> AppResult<Vec<String>> {
    // A consumer without a group id never joins a group; it is used here only
    // for metadata. Polling it serves librdkafka's queued log events.
    let consumer: BaseConsumer = ClientConfig::new()
        .set("bootstrap.servers", "localhost:9092")
        .create()?;

    let metadata = consumer.fetch_metadata(None, Duration::from_secs(10));
    let _ = consumer.poll(Duration::from_millis(50));
    let metadata = metadata.inspect_err(|error| tracing::warn!(%error, "listing topics failed"))?;

    let mut topics: Vec<String> = metadata
        .topics()
        .iter()
        .map(|topic| topic.name().to_string())
        .collect();
    topics.sort();
    tracing::debug!(count = topics.len(), "listed topics");

    Ok(topics)
}
