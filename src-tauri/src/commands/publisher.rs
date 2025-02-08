use std::time::Duration;

use rdkafka::{producer::{FutureProducer, FutureRecord}, ClientConfig};


#[tauri::command]
pub async fn send_message(message: String) -> Result<String, String> {
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", "localhost:9092")
        // Additional configuration options can be set here (e.g. security)
        .create()
        .expect("Producer creation error");

    let topic = "my_topic";
    let payload = &message;
    let key = "example_key";

    // Send the message and await delivery report
    match producer
        .send(
            FutureRecord::to(topic).payload(payload).key(key),
            Duration::from_secs(0),
        )
        .await
    {
        Ok(delivery) => {
            println!("Message delivered: {:?}", delivery);
            Ok("Message delivered successfully".into())
        }
        Err((e, _message)) => {
            eprintln!("Error delivering message: {:?}", e);
            Err("Error delivering message".into())
        }
    }
}
