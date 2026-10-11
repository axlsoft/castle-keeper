use std::time::Duration;

use rdkafka::{
    producer::{FutureProducer, FutureRecord},
    ClientConfig,
};

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
pub async fn send_message(message: PublishMessage) -> Result<PublisherResult, String> {
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", "localhost:9092")
        .create()
        .expect("Producer creation error");

    let topic = "my_topic";

    // Depending on the message type, you might serialize 'value' differently.
    // For now, we assume the 'value' is already formatted correctly.
    match producer
        .send(
            FutureRecord::to(topic)
                .payload(&message.value)
                .key(&message.key),
            Duration::from_secs(0),
        )
        .await
    {
        Ok(delivery) => {
            let (partition, offset) = (delivery.partition, delivery.offset);
            println!(
                "Message delivered: partition: {}, offset: {}",
                partition, offset
            );
            Ok(PublisherResult { partition, offset })
        }
        Err((e, _message)) => {
            eprintln!("Error delivering message: {:?}", e);
            Err("Error delivering message".into())
        }
    }
}
