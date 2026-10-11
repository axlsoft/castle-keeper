//! Checks the app's view of the data `scripts/seed.sh` creates. Skipped
//! unless `CK_IT_SEEDED=1` and `CK_IT_BOOTSTRAP` are set (the integration
//! script seeds first).

mod common;

use std::{sync::atomic::AtomicBool, time::Duration};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use castle_keeper_lib::kafka::{
    browse::{browse, BrowseEvent, BrowseRequest, StartPosition},
    groups::list_groups,
    manager::ClientManager,
    settings::ConnectionSettings,
    topics::{list_topics, topic_configs, ConfigOrigin},
};
use rdkafka::consumer::Consumer;

#[tokio::test]
async fn seeded_cluster_looks_right() {
    if std::env::var("CK_IT_SEEDED").as_deref() != Ok("1") {
        eprintln!("CK_IT_SEEDED not set; skipping");
        return;
    }
    let Some(bootstrap) = common::live_bootstrap() else {
        return;
    };
    let manager = ClientManager::default();
    let clients = manager
        .clients(&ConnectionSettings::new("seeded", &bootstrap))
        .unwrap();

    let topics = list_topics(clients.clone(), false).await.unwrap();
    let partitions = |name: &str| topics.iter().find(|t| t.name == name).map(|t| t.partitions);
    assert_eq!(partitions("orders"), Some(6));
    assert_eq!(partitions("metrics.raw"), Some(12));
    assert_eq!(partitions("customers"), Some(3));
    assert!(topics.iter().all(|t| t.under_replicated == 0));

    let configs = topic_configs(clients.clone(), "customers".into())
        .await
        .unwrap();
    let cleanup = configs.iter().find(|c| c.name == "cleanup.policy").unwrap();
    assert_eq!(
        (cleanup.origin, cleanup.value.as_deref()),
        (ConfigOrigin::TopicOverride, Some("compact"))
    );

    let groups = list_groups(clients.clone()).await.unwrap();
    let group = |id: &str| {
        groups
            .iter()
            .find(|g| g.group_id == id)
            .unwrap_or_else(|| panic!("{id} missing"))
    };
    assert!(
        group("billing-service").total_lag.unwrap() > 0,
        "billing-service should lag"
    );
    assert_eq!(group("analytics").total_lag, Some(0));
    assert_eq!(
        (
            group("reporting").state.as_str(),
            group("reporting").total_lag
        ),
        ("Empty", Some(0))
    );
    let groups_before: Vec<_> = groups.iter().map(|g| g.group_id.clone()).collect();

    // Read the latest orders as JSON, with headers.
    let request = BrowseRequest {
        topic: "orders".into(),
        partitions: vec![],
        start: StartPosition::Latest { count: 100 },
        limit: 100,
    };
    let cancel = AtomicBool::new(false);
    let mut messages = Vec::new();
    browse(&clients, &request, &cancel, &mut |event| {
        if let BrowseEvent::Batch { messages: batch } = event {
            messages.extend(batch);
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(messages.len(), 100);
    let first = &messages[0];
    let value: serde_json::Value =
        serde_json::from_slice(&BASE64.decode(first.value.as_ref().unwrap()).unwrap()).unwrap();
    assert!(value.get("orderId").is_some());
    assert!(first.headers.iter().any(|h| h.key == "trace-id"));

    let mut after: Vec<String> = clients
        .metadata_client()
        .fetch_group_list(None, Duration::from_secs(5))
        .unwrap()
        .groups()
        .iter()
        .map(|g| g.name().to_string())
        .collect();
    after.sort();
    let mut before = groups_before;
    before.sort();
    assert_eq!(after, before, "browsing must not create consumer groups");
}
