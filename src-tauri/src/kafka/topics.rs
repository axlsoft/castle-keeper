use std::{collections::BTreeSet, sync::Arc};

use rdkafka::{
    admin::{AdminOptions, ConfigSource, ResourceSpecifier},
    consumer::Consumer,
};
use serde::Serialize;
use specta::Type;

use super::{
    groups,
    manager::ClusterClients,
    offsets::{self, PartitionKey},
    run_blocking,
};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TopicSummary {
    pub name: String,
    pub internal: bool,
    pub partitions: u32,
    /// Replicas per partition (the most common count).
    pub replication_factor: u32,
    /// Partitions whose in-sync replica set is smaller than the replica set.
    pub under_replicated: u32,
    /// Partitions without a leader.
    pub offline: u32,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PartitionInfo {
    pub id: i32,
    /// -1 when the partition has no leader.
    pub leader: i32,
    pub replicas: Vec<i32>,
    pub isr: Vec<i32>,
    pub low: i64,
    pub high: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ConfigOrigin {
    TopicOverride,
    DynamicBroker,
    DynamicDefaultBroker,
    StaticBroker,
    Default,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TopicConfigEntry {
    pub name: String,
    /// Null for sensitive values.
    pub value: Option<String>,
    pub origin: ConfigOrigin,
    pub is_default: bool,
    pub sensitive: bool,
    pub read_only: bool,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TopicConsumer {
    pub group_id: String,
    pub state: String,
    /// Lag summed over this topic's partitions; null without commits.
    pub lag: Option<i64>,
    pub members: u32,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TopicDetail {
    pub summary: TopicSummary,
    pub partitions: Vec<PartitionInfo>,
    /// Total messages currently retained (sum of high − low).
    pub message_count: i64,
}

/// Topic names in the cluster, sorted. Internal topics (`__consumer_offsets`
/// and friends) are included only when asked for.
pub async fn list_topic_names(
    clients: Arc<ClusterClients>,
    include_internal: bool,
) -> AppResult<Vec<String>> {
    let timeout = clients.settings().admin_timeout;
    let mut names = run_blocking("listing topics", timeout, move || {
        let metadata = clients.metadata_client().fetch_metadata(None, timeout)?;
        Ok(metadata
            .topics()
            .iter()
            .map(|topic| topic.name().to_string())
            .collect::<Vec<_>>())
    })
    .await?;
    if !include_internal {
        names.retain(|name| !is_internal_topic(name));
    }
    names.sort();
    Ok(names)
}

pub fn is_internal_topic(name: &str) -> bool {
    name.starts_with("__")
}

fn summarize(topic: &rdkafka::metadata::MetadataTopic) -> TopicSummary {
    let partitions = topic.partitions();
    let mut rf_counts = std::collections::HashMap::<usize, u32>::new();
    for p in partitions {
        *rf_counts.entry(p.replicas().len()).or_default() += 1;
    }
    let replication_factor = rf_counts
        .into_iter()
        .max_by_key(|&(rf, count)| (count, rf))
        .map(|(rf, _)| u32::try_from(rf).unwrap_or(u32::MAX))
        .unwrap_or(0);
    let count = |pred: &dyn Fn(&rdkafka::metadata::MetadataPartition) -> bool| {
        u32::try_from(partitions.iter().filter(|p| pred(p)).count()).unwrap_or(u32::MAX)
    };
    TopicSummary {
        name: topic.name().to_string(),
        internal: is_internal_topic(topic.name()),
        partitions: u32::try_from(partitions.len()).unwrap_or(u32::MAX),
        replication_factor,
        under_replicated: count(&|p| p.isr().len() < p.replicas().len()),
        offline: count(&|p| p.leader() < 0),
    }
}

/// Every topic with partition count, replication factor and health, from a
/// single metadata request. Sorted by name.
pub async fn list_topics(
    clients: Arc<ClusterClients>,
    include_internal: bool,
) -> AppResult<Vec<TopicSummary>> {
    let timeout = clients.settings().admin_timeout;
    let mut topics = run_blocking("listing topics", timeout, move || {
        let metadata = clients.metadata_client().fetch_metadata(None, timeout)?;
        Ok(metadata.topics().iter().map(summarize).collect::<Vec<_>>())
    })
    .await?;
    if !include_internal {
        topics.retain(|t| !t.internal);
    }
    topics.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(topics)
}

/// Partitions with leader, replicas, ISR and watermarks.
pub async fn describe_topic(clients: Arc<ClusterClients>, name: String) -> AppResult<TopicDetail> {
    let timeout = clients.settings().admin_timeout;
    run_blocking("describing the topic", timeout * 2, move || {
        let metadata = clients
            .metadata_client()
            .fetch_metadata(Some(&name), timeout)?;
        let topic = metadata
            .topics()
            .iter()
            .find(|t| t.name() == name)
            .ok_or_else(|| AppError::not_found(format!("Topic {name}")))?;
        if let Some(error) = topic.error() {
            return Err(rdkafka::error::KafkaError::MetadataFetch(error.into()).into());
        }
        let keys: Vec<PartitionKey> = topic
            .partitions()
            .iter()
            .map(|p| (name.clone(), p.id()))
            .collect();
        let marks = offsets::watermarks(clients.metadata_client(), &keys, timeout)?;
        let mut partitions: Vec<PartitionInfo> = topic
            .partitions()
            .iter()
            .map(|p| {
                let (low, high) = marks
                    .get(&(name.clone(), p.id()))
                    .copied()
                    .unwrap_or((0, 0));
                PartitionInfo {
                    id: p.id(),
                    leader: p.leader(),
                    replicas: p.replicas().to_vec(),
                    isr: p.isr().to_vec(),
                    low,
                    high,
                }
            })
            .collect();
        partitions.sort_by_key(|p| p.id);
        let message_count = partitions.iter().map(|p| (p.high - p.low).max(0)).sum();
        Ok(TopicDetail {
            summary: summarize(topic),
            partitions,
            message_count,
        })
    })
    .await
}

/// Topic configuration with where each value comes from.
pub async fn topic_configs(
    clients: Arc<ClusterClients>,
    name: String,
) -> AppResult<Vec<TopicConfigEntry>> {
    let timeout = clients.settings().admin_timeout;
    let options = AdminOptions::new().request_timeout(Some(timeout));
    let results = tokio::time::timeout(
        timeout + std::time::Duration::from_secs(1),
        clients
            .admin()
            .describe_configs(&[ResourceSpecifier::Topic(&name)], &options),
    )
    .await
    .map_err(|_| AppError::timeout("reading topic configuration", timeout))??;
    let resource = results
        .into_iter()
        .next()
        .ok_or_else(|| AppError::not_found(format!("Topic {name}")))?
        .map_err(rdkafka::error::KafkaError::AdminOp)?;
    let mut entries: Vec<TopicConfigEntry> = resource
        .entries
        .into_iter()
        .map(|e| TopicConfigEntry {
            value: if e.is_sensitive { None } else { e.value },
            origin: match e.source {
                ConfigSource::DynamicTopic => ConfigOrigin::TopicOverride,
                ConfigSource::DynamicBroker => ConfigOrigin::DynamicBroker,
                ConfigSource::DynamicDefaultBroker => ConfigOrigin::DynamicDefaultBroker,
                ConfigSource::StaticBroker => ConfigOrigin::StaticBroker,
                ConfigSource::Default => ConfigOrigin::Default,
                ConfigSource::Unknown => ConfigOrigin::Unknown,
            },
            name: e.name,
            is_default: e.is_default,
            sensitive: e.is_sensitive,
            read_only: e.is_read_only,
        })
        .collect();
    // Overrides first, then alphabetical.
    entries.sort_by(|a, b| {
        (a.origin != ConfigOrigin::TopicOverride, &a.name)
            .cmp(&(b.origin != ConfigOrigin::TopicOverride, &b.name))
    });
    Ok(entries)
}

/// Consumer groups that read this topic (committed offsets or a current
/// assignment), with their lag on it.
pub async fn topic_consumers(
    clients: Arc<ClusterClients>,
    name: String,
) -> AppResult<Vec<TopicConsumer>> {
    let summaries = groups::list_groups(clients.clone()).await?;
    let mut out = Vec::new();
    for summary in summaries.into_iter().filter(|g| g.topics.contains(&name)) {
        let detail = groups::describe_group(clients.clone(), summary.group_id.clone()).await?;
        let on_topic: Vec<_> = detail
            .partitions
            .iter()
            .filter(|p| p.topic == name)
            .collect();
        let lag = on_topic
            .iter()
            .any(|p| p.lag.is_some())
            .then(|| on_topic.iter().filter_map(|p| p.lag).sum());
        let members: BTreeSet<&str> = on_topic
            .iter()
            .filter_map(|p| p.member_id.as_deref())
            .collect();
        out.push(TopicConsumer {
            group_id: summary.group_id,
            state: summary.state,
            lag,
            members: u32::try_from(members.len()).unwrap_or(u32::MAX),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::is_internal_topic;

    #[test]
    fn internal_topics_are_double_underscore_prefixed() {
        assert!(is_internal_topic("__consumer_offsets"));
        assert!(is_internal_topic("__transaction_state"));
        assert!(!is_internal_topic("orders"));
        assert!(!is_internal_topic("_schemas"));
    }
}
