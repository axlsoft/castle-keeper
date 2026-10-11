//! Consumer group listing and lag math against the local broker. Skipped
//! unless `CK_IT_BOOTSTRAP` is set.

mod common;

use std::time::Duration;

use castle_keeper_lib::kafka::{
    groups::{describe_group, list_groups},
    manager::ClientManager,
    settings::ConnectionSettings,
};
use rdkafka::{
    admin::{AdminOptions, NewTopic, TopicReplication},
    consumer::{BaseConsumer, CommitMode, Consumer},
    producer::{FutureProducer, FutureRecord},
    ClientConfig, Offset, TopicPartitionList,
};

const TOPIC: &str = "ck-it-groups";
const LAGGING: &str = "ck-it-groups-lagging";
const CAUGHT_UP: &str = "ck-it-groups-caught-up";

async fn commit(bootstrap: &str, group: &str, offsets: &[(i32, i64)]) {
    let consumer: BaseConsumer = ClientConfig::new()
        .set("bootstrap.servers", bootstrap)
        .set("group.id", group)
        .create()
        .unwrap();
    let mut tpl = TopicPartitionList::new();
    for (partition, offset) in offsets {
        tpl.add_partition_offset(TOPIC, *partition, Offset::Offset(*offset))
            .unwrap();
    }
    for _ in 0..20 {
        if consumer.commit(&tpl, CommitMode::Sync).is_ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    panic!("could not commit for {group}");
}

#[tokio::test]
async fn lag_is_log_end_minus_committed() {
    let Some(bootstrap) = common::live_bootstrap() else {
        return;
    };
    let manager = ClientManager::default();
    let clients = manager
        .clients(&ConnectionSettings::new("groups", &bootstrap))
        .unwrap();
    let _ = clients
        .admin()
        .create_topics(
            &[NewTopic::new(TOPIC, 2, TopicReplication::Fixed(1))],
            &AdminOptions::new(),
        )
        .await;

    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &bootstrap)
        .create()
        .unwrap();
    for i in 0..20 {
        producer
            .send(
                FutureRecord::to(TOPIC)
                    .partition(i % 2)
                    .key("k")
                    .payload("v"),
                Duration::from_secs(5),
            )
            .await
            .unwrap();
    }
    let ends: Vec<i64> = (0..2)
        .map(|p| {
            clients
                .metadata_client()
                .fetch_watermarks(TOPIC, p, Duration::from_secs(5))
                .unwrap()
                .1
        })
        .collect();

    commit(&bootstrap, LAGGING, &[(0, 3), (1, 5)]).await;
    commit(&bootstrap, CAUGHT_UP, &[(0, ends[0]), (1, ends[1])]).await;

    let detail = describe_group(clients.clone(), LAGGING.into())
        .await
        .unwrap();
    assert_eq!(detail.state, "Empty");
    let p0 = detail
        .partitions
        .iter()
        .find(|p| p.topic == TOPIC && p.partition == 0)
        .unwrap();
    assert_eq!(
        (p0.committed, p0.log_end, p0.lag),
        (Some(3), ends[0], Some(ends[0] - 3))
    );
    assert_eq!(detail.total_lag, Some(ends[0] - 3 + ends[1] - 5));

    let summaries = list_groups(clients.clone()).await.unwrap();
    let lagging = summaries.iter().find(|g| g.group_id == LAGGING).unwrap();
    assert_eq!(lagging.total_lag, detail.total_lag);
    assert_eq!(lagging.topics, vec![TOPIC.to_string()]);
    let caught_up = summaries.iter().find(|g| g.group_id == CAUGHT_UP).unwrap();
    assert_eq!(caught_up.total_lag, Some(0));

    let missing = describe_group(clients, "ck-it-no-such-group".into()).await;
    assert!(missing.is_err() || missing.unwrap().partitions.is_empty());
}
