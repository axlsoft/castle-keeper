use std::time::Duration;

use rdkafka::{admin::AdminClient, client::DefaultClientContext, ClientConfig};

use crate::error::AppResult;

#[tauri::command]
pub async fn get_topics() -> AppResult<Vec<String>> {
    let admin_client: AdminClient<DefaultClientContext> = ClientConfig::new()
        .set("bootstrap.servers", "localhost:9092")
        .create()?;

    let metadata = admin_client
        .inner()
        .fetch_metadata(None, Duration::from_secs(10))?;

    let mut topics: Vec<String> = metadata
        .topics()
        .iter()
        .map(|topic| topic.name().to_string())
        .collect();
    topics.sort();

    Ok(topics)
}
