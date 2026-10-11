//! Topic summaries, partition details, watermarks, configs and consumers
//! against the local broker. Skipped unless `CK_IT_BOOTSTRAP` is set.

mod common;

use std::time::Duration;

use castle_keeper_lib::kafka::{
    manager::ClientManager,
    offsets::watermarks,
    settings::ConnectionSettings,
    topics::{describe_topic, list_topics, topic_configs, topic_consumers, ConfigOrigin},
};
use rdkafka::{
    admin::{AdminOptions, NewTopic, TopicReplication},
    consumer::{BaseConsumer, Consumer},
    producer::{FutureProducer, FutureRecord},
    ClientConfig, Offset, TopicPartitionList,
};

const TOPIC: &str = "ck-it-topics";
const GROUP: &str = "ck-it-topics-group";

async fn seed(bootstrap: &str) {
    let manager = ClientManager::default();
    let clients = manager
        .clients(&ConnectionSettings::new("seed", bootstrap))
        .unwrap();
    let topic = NewTopic::new(TOPIC, 3, TopicReplication::Fixed(1)).set("retention.ms", "3600000");
    let _ = clients
        .admin()
        .create_topics(&[topic], &AdminOptions::new())
        .await;

    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", bootstrap)
        .create()
        .unwrap();
    for i in 0..30 {
        let payload = format!("{{\"n\":{i}}}");
        producer
            .send(
                FutureRecord::to(TOPIC)
                    .partition(i % 3)
                    .key("k")
                    .payload(&payload),
                Duration::from_secs(5),
            )
            .await
            .unwrap();
    }

    // A group that committed offset 4 on partition 0 only.
    let consumer: BaseConsumer = ClientConfig::new()
        .set("bootstrap.servers", bootstrap)
        .set("group.id", GROUP)
        .create()
        .unwrap();
    let mut tpl = TopicPartitionList::new();
    tpl.add_partition_offset(TOPIC, 0, Offset::Offset(4))
        .unwrap();
    consumer
        .commit(&tpl, rdkafka::consumer::CommitMode::Sync)
        .unwrap();
}

#[tokio::test]
async fn topic_views_against_live_broker() {
    let Some(bootstrap) = common::live_bootstrap() else {
        return;
    };
    seed(&bootstrap).await;
    let manager = ClientManager::default();
    let clients = manager
        .clients(&ConnectionSettings::new("topics", &bootstrap))
        .unwrap();

    let summaries = list_topics(clients.clone(), false).await.unwrap();
    let summary = summaries
        .iter()
        .find(|t| t.name == TOPIC)
        .expect("seeded topic listed");
    assert_eq!(
        (
            summary.partitions,
            summary.replication_factor,
            summary.under_replicated
        ),
        (3, 1, 0)
    );

    let detail = describe_topic(clients.clone(), TOPIC.into()).await.unwrap();
    assert_eq!(detail.partitions.len(), 3);
    assert!(detail
        .partitions
        .iter()
        .all(|p| p.leader == 1 && p.isr == vec![1]));
    assert!(detail.message_count >= 30);

    // The batched lookup agrees with per-partition watermark queries.
    let keys: Vec<_> = (0..3).map(|p| (TOPIC.to_string(), p)).collect();
    let batched = watermarks(clients.metadata_client(), &keys, Duration::from_secs(5)).unwrap();
    for (topic, partition) in &keys {
        let direct = clients
            .metadata_client()
            .fetch_watermarks(topic, *partition, Duration::from_secs(5))
            .unwrap();
        assert_eq!(batched[&(topic.clone(), *partition)], direct);
    }

    let configs = topic_configs(clients.clone(), TOPIC.into()).await.unwrap();
    let retention = configs.iter().find(|c| c.name == "retention.ms").unwrap();
    assert_eq!(retention.origin, ConfigOrigin::TopicOverride);
    assert_eq!(retention.value.as_deref(), Some("3600000"));
    assert_eq!(
        configs[0].origin,
        ConfigOrigin::TopicOverride,
        "overrides sort first"
    );
    assert!(configs.iter().any(|c| c.origin == ConfigOrigin::Default));

    let consumers = topic_consumers(clients, TOPIC.into()).await.unwrap();
    let group = consumers
        .iter()
        .find(|c| c.group_id == GROUP)
        .expect("committed group listed");
    let p0_high = batched[&(TOPIC.to_string(), 0)].1;
    assert_eq!(group.lag, Some(p0_high - 4));
}
