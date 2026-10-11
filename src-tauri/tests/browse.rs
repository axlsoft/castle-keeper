//! Message browsing against the local broker. Skipped unless
//! `CK_IT_BOOTSTRAP` is set.

mod common;

use std::{
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use castle_keeper_lib::kafka::{
    browse::{BrowseEvent, BrowseRequest, BrowsedMessage, StartPosition, StopReason, browse},
    manager::{ClientManager, ClusterClients},
    settings::ConnectionSettings,
};
use rdkafka::{
    ClientConfig,
    admin::{AdminOptions, NewTopic, TopicReplication},
    consumer::Consumer,
    message::{Header, OwnedHeaders},
    producer::{FutureProducer, FutureRecord},
};

const PER_PARTITION: i32 = 20;

fn now_ms() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap()
}

/// Seeds a fresh topic; returns the timestamp just before the second half
/// of the messages was produced.
async fn seed(bootstrap: &str, clients: &ClusterClients, topic: &str) -> i64 {
    let created = clients
        .admin()
        .create_topics(
            &[NewTopic::new(topic, 3, TopicReplication::Fixed(1))],
            &AdminOptions::new(),
        )
        .await
        .unwrap();
    for result in created {
        result.unwrap();
    }
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", bootstrap)
        .create()
        .unwrap();
    let mut midpoint = now_ms();
    for i in 0..PER_PARTITION * 3 {
        if i == PER_PARTITION * 3 / 2 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            midpoint = now_ms();
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let payload = format!("{{\"order\":{i},\"note\":\"<b>not html</b>\"}}");
        let headers = OwnedHeaders::new().insert(Header {
            key: "source",
            value: Some("it"),
        });
        producer
            .send(
                FutureRecord::to(topic)
                    .partition(i % 3)
                    .key(&format!("k{i}"))
                    .payload(&payload)
                    .headers(headers),
                Duration::from_secs(5),
            )
            .await
            .unwrap();
    }
    midpoint
}

fn run(clients: &ClusterClients, request: BrowseRequest) -> (Vec<BrowsedMessage>, StopReason) {
    let cancel = AtomicBool::new(false);
    let mut messages = Vec::new();
    let mut reason = None;
    browse(clients, &request, &cancel, &mut |event| {
        match event {
            BrowseEvent::Batch { messages: batch } => messages.extend(batch),
            BrowseEvent::Done { reason: r, .. } => reason = Some(r),
            BrowseEvent::Started { .. } => {}
        }
        Ok(())
    })
    .unwrap();
    (messages, reason.unwrap())
}

fn group_names(clients: &ClusterClients) -> Vec<String> {
    let mut names: Vec<String> = clients
        .metadata_client()
        .fetch_group_list(None, Duration::from_secs(5))
        .unwrap()
        .groups()
        .iter()
        .map(|g| g.name().to_string())
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn browsing_reads_ranges_and_never_creates_a_group() {
    let Some(bootstrap) = common::live_bootstrap() else {
        return;
    };
    let manager = ClientManager::default();
    let clients = manager
        .clients(&ConnectionSettings::new("browse", &bootstrap))
        .unwrap();
    // A fresh topic per run keeps the test repeatable.
    let topic = format!("ck-it-browse-{}", now_ms());
    let topic = topic.as_str();
    let midpoint = seed(&bootstrap, &clients, topic).await;
    let groups_before = group_names(&clients);
    let owned_topic = topic.to_string();
    let request = |start, partitions: Vec<i32>, limit| BrowseRequest {
        topic: owned_topic.clone(),
        partitions,
        start,
        limit,
    };

    let (all, reason) = run(&clients, request(StartPosition::Earliest, vec![], 10_000));
    assert_eq!(reason, StopReason::Caught);
    assert!(all.len() >= (PER_PARTITION * 3) as usize, "{}", all.len());
    let first = all
        .iter()
        .find(|m| m.partition == 0 && m.offset == 0)
        .unwrap();
    let value = String::from_utf8(BASE64.decode(first.value.as_ref().unwrap()).unwrap()).unwrap();
    assert!(value.contains("<b>not html</b>"));
    assert_eq!(first.headers[0].key, "source");
    assert_eq!(
        BASE64
            .decode(first.headers[0].value.as_ref().unwrap())
            .unwrap(),
        b"it"
    );

    let (latest, _) = run(
        &clients,
        request(StartPosition::Latest { count: 5 }, vec![], 10_000),
    );
    assert_eq!(latest.len(), 15);
    for p in 0..3 {
        let high = clients
            .metadata_client()
            .fetch_watermarks(topic, p, Duration::from_secs(5))
            .unwrap()
            .1;
        assert!(
            latest
                .iter()
                .filter(|m| m.partition == p)
                .all(|m| m.offset >= high - 5)
        );
    }

    let (from_offset, _) = run(
        &clients,
        request(StartPosition::Offset { offset: 15 }, vec![1], 10_000),
    );
    assert!(
        from_offset
            .iter()
            .all(|m| m.partition == 1 && m.offset >= 15)
    );
    assert_eq!(from_offset.first().map(|m| m.offset), Some(15));

    let (limited, reason) = run(&clients, request(StartPosition::Earliest, vec![], 7));
    assert_eq!((limited.len(), reason), (7, StopReason::Limit));

    let (by_time, _) = run(
        &clients,
        request(
            StartPosition::Timestamp {
                timestamp: midpoint,
            },
            vec![],
            10_000,
        ),
    );
    assert!(!by_time.is_empty() && by_time.len() < all.len());
    assert!(by_time.iter().all(|m| m.timestamp.unwrap() >= midpoint));

    // Cancelling stops at once.
    let cancel = Arc::new(AtomicBool::new(true));
    let mut done = None;
    browse(
        &clients,
        &request(StartPosition::Earliest, vec![], 10_000),
        &cancel,
        &mut |event| {
            if let BrowseEvent::Done { reason, .. } = event {
                done = Some(reason);
            }
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(done, Some(StopReason::Cancelled));

    let bad = BrowseRequest {
        topic: topic.into(),
        partitions: vec![99],
        start: StartPosition::Earliest,
        limit: 10,
    };
    let cancel = AtomicBool::new(false);
    assert!(browse(&clients, &bad, &cancel, &mut |_| Ok(())).is_err());

    assert_eq!(
        group_names(&clients),
        groups_before,
        "browsing must not create consumer groups"
    );
}
