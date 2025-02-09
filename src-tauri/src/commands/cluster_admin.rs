use std::time::Duration;

use rdkafka::{admin::AdminClient, client::DefaultClientContext, ClientConfig};

#[tauri::command]
pub async fn get_topics() -> Result<Vec<String>, String> {
    let admin_client: AdminClient<DefaultClientContext> = ClientConfig::new()
        .set("bootstrap.servers", "localhost:9092")
        .create()
        .expect("Admin client creation error");

    let metadata = admin_client
        .inner()
        .fetch_metadata(None, Duration::from_secs(10))
        .expect("Failed to fetch metadata");

    let topics: Vec<String> = metadata
        .topics()
        .iter()
        .map(|topic| topic.name().to_string())
        .collect();

    Ok(topics)
}
