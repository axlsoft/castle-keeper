//! Client manager behaviour: pooling, timeouts and not blocking the async
//! runtime. Tests that need a broker are skipped unless `CK_IT_BOOTSTRAP` is
//! set (e.g. `127.0.0.1:9092` from `docker compose up -d --wait`); the
//! broker-down tests use a closed local port and always run.

mod common;

use std::time::{Duration, Instant};

use castle_keeper_lib::{
    audit::{Outcome, WriteAction},
    error::ErrorKind,
    guard::WriteRequest,
    kafka::{
        manager::ClientManager,
        produce::{produce, ProduceRequest},
        settings::ConnectionSettings,
        topics::list_topic_names,
    },
    profiles::Environment,
};
use common::{down_settings, guard, live_bootstrap, profile, ticket};

#[test]
fn broker_down_metadata_fails_in_time_without_stalling_the_runtime() {
    // One worker thread: if the blocking metadata call ran on it, the ticker
    // below would stall for the whole timeout.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let ticker = tokio::spawn(async {
            let mut worst = Duration::ZERO;
            let started = Instant::now();
            let mut last = Instant::now();
            while started.elapsed() < Duration::from_secs(3) {
                tokio::time::sleep(Duration::from_millis(10)).await;
                worst = worst.max(last.elapsed());
                last = Instant::now();
            }
            worst
        });

        let manager = ClientManager::default();
        let clients = manager.clients(&down_settings("down-metadata")).unwrap();
        let started = Instant::now();
        let error = list_topic_names(clients, false).await.unwrap_err();
        let elapsed = started.elapsed();

        assert!(
            matches!(error.kind, ErrorKind::Unreachable | ErrorKind::Timeout),
            "unexpected error: {error:?}"
        );
        assert!(elapsed < Duration::from_millis(3_000), "took {elapsed:?}");
        let worst_gap = ticker.await.unwrap();
        assert!(
            worst_gap < Duration::from_millis(100),
            "runtime stalled for {worst_gap:?}"
        );
    });
}

#[tokio::test]
async fn broker_down_produce_fails_within_the_delivery_timeout() {
    let manager = ClientManager::default();
    let clients = manager.clients(&down_settings("down-produce")).unwrap();
    let request = ProduceRequest {
        topic: "anything".into(),
        partition: None,
        key: None,
        value: "v".into(),
        headers: vec![],
    };
    let dir = tempfile::tempdir().unwrap();
    let guard = guard(&dir);
    let started = Instant::now();
    let error = produce(&clients, &request, ticket(&guard, "anything", false))
        .await
        .unwrap_err();
    assert!(
        matches!(error.kind, ErrorKind::Timeout | ErrorKind::Unreachable),
        "unexpected error: {error:?}"
    );
    assert!(
        started.elapsed() < Duration::from_millis(3_500),
        "took {:?}",
        started.elapsed()
    );
    let logged = guard.log().recent(1).unwrap();
    assert_eq!(logged[0].outcome, Outcome::Failed);
}

#[tokio::test]
async fn read_only_profile_blocks_produce_and_logs_it() {
    let dir = tempfile::tempdir().unwrap();
    let guard = guard(&dir);
    let prod = profile(Environment::Prod, true);
    let error = guard
        .authorize(
            &prod,
            WriteRequest {
                action: WriteAction::Produce,
                target: "orders".into(),
                dry_run: false,
                confirmation: Some("it-profile".into()),
            },
        )
        .err()
        .unwrap();
    assert_eq!(error.kind, ErrorKind::ReadOnly);
    let logged = guard.log().recent(1).unwrap();
    assert_eq!(
        (logged[0].outcome, logged[0].target.as_str()),
        (Outcome::Blocked, "orders")
    );
}

#[tokio::test]
async fn dry_run_previews_without_sending() {
    let dir = tempfile::tempdir().unwrap();
    let guard = guard(&dir);
    let manager = ClientManager::default();
    // Unreachable broker: a dry run must not touch the network.
    let clients = manager.clients(&down_settings("dry-run")).unwrap();
    let request = ProduceRequest {
        topic: "orders".into(),
        partition: Some(2),
        key: Some("k1".into()),
        value: "{}".into(),
        headers: vec![("h".into(), "v".into())],
    };
    let started = Instant::now();
    let outcome = produce(&clients, &request, ticket(&guard, "orders", true))
        .await
        .unwrap();
    assert!(started.elapsed() < Duration::from_millis(200));
    assert!(outcome.dry_run);
    assert_eq!(
        (
            outcome.offset,
            outcome.key_bytes,
            outcome.value_bytes,
            outcome.header_count
        ),
        (None, 2, 2, 1)
    );
    assert_eq!(guard.log().recent(1).unwrap()[0].outcome, Outcome::DryRun);
}

#[tokio::test]
async fn ticket_for_another_topic_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let guard = guard(&dir);
    let manager = ClientManager::default();
    let clients = manager.clients(&down_settings("mismatch")).unwrap();
    let request = ProduceRequest {
        topic: "payments".into(),
        partition: None,
        key: None,
        value: "v".into(),
        headers: vec![],
    };
    let error = produce(&clients, &request, ticket(&guard, "orders", false))
        .await
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Internal);
}

#[tokio::test]
async fn second_metadata_call_reuses_the_connection() {
    let Some(bootstrap) = live_bootstrap() else {
        return;
    };
    let manager = ClientManager::default();
    let settings = ConnectionSettings::new("live", bootstrap);

    let first = manager.clients(&settings).unwrap();
    list_topic_names(first, false).await.unwrap();

    let started = Instant::now();
    let again = manager.clients(&settings).unwrap();
    list_topic_names(again, false).await.unwrap();
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_millis(50),
        "second call took {elapsed:?}"
    );
}

#[tokio::test]
async fn produce_round_trip_and_topic_listing() {
    let Some(bootstrap) = live_bootstrap() else {
        return;
    };
    let manager = ClientManager::default();
    let clients = manager
        .clients(&ConnectionSettings::new("live-produce", bootstrap))
        .unwrap();
    let topic = "ck-it-client-manager";
    let dir = tempfile::tempdir().unwrap();
    let guard = guard(&dir);

    let request = ProduceRequest {
        topic: topic.into(),
        partition: None,
        key: Some("k".into()),
        value: "{\"hello\":\"world\"}".into(),
        headers: vec![("source".into(), "integration-test".into())],
    };
    // The first produce may race topic auto-creation; retry briefly.
    let mut delivered = None;
    for _ in 0..10 {
        match produce(&clients, &request, ticket(&guard, topic, false)).await {
            Ok(result) => {
                delivered = Some(result);
                break;
            }
            Err(_) => tokio::time::sleep(Duration::from_millis(500)).await,
        }
    }
    let delivered = delivered.expect("message was never delivered");
    assert!(delivered.offset.unwrap_or(-1) >= 0);
    let logged = guard.log().recent(1).unwrap();
    assert_eq!(logged[0].outcome, Outcome::Ok);
    assert!(logged[0].detail.as_deref().unwrap_or("").contains("offset"));

    let names = list_topic_names(clients.clone(), false).await.unwrap();
    assert!(names.contains(&topic.to_string()));
    assert!(names.windows(2).all(|w| w[0] <= w[1]), "not sorted");
    assert!(
        !names.iter().any(|n| n.starts_with("__")),
        "internal topics leaked"
    );

    // __consumer_offsets only exists once a group has committed, so check
    // that the unfiltered listing is a superset rather than a specific name.
    let with_internal = list_topic_names(clients, true).await.unwrap();
    assert!(names.iter().all(|n| with_internal.contains(n)));
}
