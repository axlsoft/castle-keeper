//! Consumer groups: listing, member assignments and lag.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::Arc,
    time::Duration,
};

use rdkafka::{consumer::Consumer, groups::GroupInfo};
use serde::Serialize;
use specta::Type;

use super::{
    manager::ClusterClients,
    offsets::{self, PartitionKey},
    run_blocking,
};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GroupSummary {
    pub group_id: String,
    /// Broker-reported state: Stable, Empty, PreparingRebalance, Dead, ...
    pub state: String,
    pub protocol_type: String,
    pub members: u32,
    pub topics: Vec<String>,
    /// Sum of per-partition lag; null when the group has no committed offsets.
    pub total_lag: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GroupMember {
    pub member_id: String,
    pub client_id: String,
    pub host: String,
    pub assignment: Vec<TopicAssignment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TopicAssignment {
    pub topic: String,
    pub partitions: Vec<i32>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PartitionLag {
    pub topic: String,
    pub partition: i32,
    pub committed: Option<i64>,
    pub log_end: i64,
    /// log end − committed; null when nothing is committed.
    pub lag: Option<i64>,
    /// Member currently assigned this partition, if any.
    pub member_id: Option<String>,
    pub client_id: Option<String>,
    pub host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GroupDetail {
    pub group_id: String,
    pub state: String,
    pub protocol_type: String,
    pub protocol: String,
    pub members: Vec<GroupMember>,
    pub partitions: Vec<PartitionLag>,
    pub total_lag: Option<i64>,
}

/// Lag for one partition. Never negative: a committed offset ahead of the
/// cached log end (a race between the two reads) counts as caught up.
pub fn lag(committed: Option<i64>, log_end: i64) -> Option<i64> {
    committed.map(|c| (log_end - c).max(0))
}

/// Decodes a consumer-protocol member assignment (versions 0-3):
/// int16 version, then an array of (string topic, array of int32
/// partitions), then user data we ignore.
pub fn decode_assignment(bytes: &[u8]) -> Vec<TopicAssignment> {
    struct Reader<'a>(&'a [u8]);
    impl Reader<'_> {
        fn take(&mut self, n: usize) -> Option<&[u8]> {
            if self.0.len() < n {
                return None;
            }
            let (head, rest) = self.0.split_at(n);
            self.0 = rest;
            Some(head)
        }
        fn i16(&mut self) -> Option<i16> {
            self.take(2).map(|b| i16::from_be_bytes([b[0], b[1]]))
        }
        fn i32(&mut self) -> Option<i32> {
            self.take(4)
                .map(|b| i32::from_be_bytes([b[0], b[1], b[2], b[3]]))
        }
        fn string(&mut self) -> Option<String> {
            let len = usize::try_from(self.i16()?).ok()?;
            self.take(len)
                .map(|b| String::from_utf8_lossy(b).into_owned())
        }
    }

    let mut reader = Reader(bytes);
    let mut out = Vec::new();
    let parse = |reader: &mut Reader<'_>, out: &mut Vec<TopicAssignment>| -> Option<()> {
        reader.i16()?; // version
        let topics = reader.i32()?;
        for _ in 0..topics.max(0) {
            let topic = reader.string()?;
            let count = reader.i32()?;
            let mut partitions = Vec::with_capacity(usize::try_from(count).unwrap_or(0));
            for _ in 0..count.max(0) {
                partitions.push(reader.i32()?);
            }
            partitions.sort_unstable();
            out.push(TopicAssignment { topic, partitions });
        }
        Some(())
    };
    let _ = parse(&mut reader, &mut out);
    out
}

fn members(group: &GroupInfo) -> Vec<GroupMember> {
    group
        .members()
        .iter()
        .map(|m| GroupMember {
            member_id: m.id().to_string(),
            client_id: m.client_id().to_string(),
            host: m.client_host().to_string(),
            assignment: m.assignment().map(decode_assignment).unwrap_or_default(),
        })
        .collect()
}

/// Partitions of every topic in the cluster, keyed by topic.
fn topic_partitions(
    clients: &ClusterClients,
    timeout: Duration,
) -> AppResult<BTreeMap<String, Vec<i32>>> {
    let metadata = clients.metadata_client().fetch_metadata(None, timeout)?;
    Ok(metadata
        .topics()
        .iter()
        .map(|t| {
            (
                t.name().to_string(),
                t.partitions().iter().map(|p| p.id()).collect(),
            )
        })
        .collect())
}

/// Committed offsets of one group across all topics. OffsetFetch needs the
/// partitions up front, so this asks for every partition in the cluster and
/// keeps the ones with a committed offset.
fn group_committed(
    clients: &ClusterClients,
    group_id: &str,
    all: &[PartitionKey],
    timeout: Duration,
) -> AppResult<HashMap<PartitionKey, i64>> {
    let consumer = clients.group_consumer(group_id)?;
    let committed = offsets::committed(&consumer, all, timeout);
    // Serve queued events (logs) on the otherwise idle group consumer.
    let _ = consumer.poll(Duration::ZERO);
    committed
}

fn all_partitions(topics: &BTreeMap<String, Vec<i32>>) -> Vec<PartitionKey> {
    topics
        .iter()
        .filter(|(name, _)| !super::topics::is_internal_topic(name))
        .flat_map(|(name, parts)| parts.iter().map(move |p| (name.clone(), *p)))
        .collect()
}

/// Every consumer group with its state, members, topics and total lag.
pub async fn list_groups(clients: Arc<ClusterClients>) -> AppResult<Vec<GroupSummary>> {
    let timeout = clients.settings().admin_timeout;
    run_blocking("listing consumer groups", timeout * 3, move || {
        let groups = clients.metadata_client().fetch_group_list(None, timeout)?;
        let topics = topic_partitions(&clients, timeout)?;
        let all = all_partitions(&topics);

        let mut committed_by_group = Vec::new();
        for group in groups.groups() {
            let committed = group_committed(&clients, group.name(), &all, timeout)?;
            committed_by_group.push((group, committed));
        }

        // One batched watermark lookup for every partition any group touches.
        let touched: BTreeSet<PartitionKey> = committed_by_group
            .iter()
            .flat_map(|(_, c)| c.keys().cloned())
            .collect();
        let touched: Vec<_> = touched.into_iter().collect();
        let marks = offsets::watermarks(clients.metadata_client(), &touched, timeout)?;

        let mut out = Vec::new();
        for (group, committed) in committed_by_group {
            let members = members(group);
            let mut group_topics: BTreeSet<String> =
                committed.keys().map(|(t, _)| t.clone()).collect();
            for member in &members {
                group_topics.extend(member.assignment.iter().map(|a| a.topic.clone()));
            }
            let total_lag = (!committed.is_empty()).then(|| {
                committed
                    .iter()
                    .filter_map(|(key, &c)| {
                        marks.get(key).and_then(|&(_, high)| lag(Some(c), high))
                    })
                    .sum()
            });
            out.push(GroupSummary {
                group_id: group.name().to_string(),
                state: group.state().to_string(),
                protocol_type: group.protocol_type().to_string(),
                members: u32::try_from(members.len()).unwrap_or(u32::MAX),
                topics: group_topics.into_iter().collect(),
                total_lag,
            });
        }
        out.sort_by(|a, b| a.group_id.cmp(&b.group_id));
        Ok(out)
    })
    .await
}

/// One group's members and per-partition committed offset, log end and lag.
pub async fn describe_group(
    clients: Arc<ClusterClients>,
    group_id: String,
) -> AppResult<GroupDetail> {
    let timeout = clients.settings().admin_timeout;
    run_blocking("describing the consumer group", timeout * 3, move || {
        let groups = clients
            .metadata_client()
            .fetch_group_list(Some(&group_id), timeout)?;
        let group = groups
            .groups()
            .iter()
            .find(|g| g.name() == group_id)
            .ok_or_else(|| AppError::not_found(format!("Consumer group {group_id}")))?;
        let members = members(group);

        let topics = topic_partitions(&clients, timeout)?;
        let committed = group_committed(&clients, &group_id, &all_partitions(&topics), timeout)?;

        // Report every partition of every topic the group commits to or is
        // assigned, so unconsumed partitions show up too.
        let mut group_topics: BTreeSet<String> = committed.keys().map(|(t, _)| t.clone()).collect();
        for member in &members {
            group_topics.extend(member.assignment.iter().map(|a| a.topic.clone()));
        }
        let keys: Vec<PartitionKey> = group_topics
            .iter()
            .filter_map(|t| topics.get(t).map(|parts| (t, parts)))
            .flat_map(|(t, parts)| parts.iter().map(move |p| (t.clone(), *p)))
            .collect();
        let marks = offsets::watermarks(clients.metadata_client(), &keys, timeout)?;

        let owner: HashMap<PartitionKey, &GroupMember> = members
            .iter()
            .flat_map(|m| {
                m.assignment
                    .iter()
                    .flat_map(move |a| a.partitions.iter().map(move |p| ((a.topic.clone(), *p), m)))
            })
            .collect();

        let partitions: Vec<PartitionLag> = keys
            .iter()
            .map(|key| {
                let log_end = marks.get(key).map(|&(_, high)| high).unwrap_or(0);
                let committed = committed.get(key).copied();
                let member = owner.get(key);
                PartitionLag {
                    topic: key.0.clone(),
                    partition: key.1,
                    committed,
                    log_end,
                    lag: lag(committed, log_end),
                    member_id: member.map(|m| m.member_id.clone()),
                    client_id: member.map(|m| m.client_id.clone()),
                    host: member.map(|m| m.host.clone()),
                }
            })
            .collect();
        let total_lag = partitions
            .iter()
            .any(|p| p.lag.is_some())
            .then(|| partitions.iter().filter_map(|p| p.lag).sum());

        Ok(GroupDetail {
            group_id: group.name().to_string(),
            state: group.state().to_string(),
            protocol_type: group.protocol_type().to_string(),
            protocol: group.protocol().to_string(),
            members,
            partitions,
            total_lag,
        })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(version: i16, topics: &[(&str, &[i32])]) -> Vec<u8> {
        let mut out = version.to_be_bytes().to_vec();
        out.extend((topics.len() as i32).to_be_bytes());
        for (topic, parts) in topics {
            out.extend((topic.len() as i16).to_be_bytes());
            out.extend(topic.as_bytes());
            out.extend((parts.len() as i32).to_be_bytes());
            for p in *parts {
                out.extend(p.to_be_bytes());
            }
        }
        out.extend((-1i32).to_be_bytes()); // null user data
        out
    }

    #[test]
    fn decodes_consumer_assignments() {
        let bytes = encode(3, &[("orders", &[2, 0]), ("payments", &[1])]);
        assert_eq!(
            decode_assignment(&bytes),
            vec![
                TopicAssignment {
                    topic: "orders".into(),
                    partitions: vec![0, 2]
                },
                TopicAssignment {
                    topic: "payments".into(),
                    partitions: vec![1]
                },
            ]
        );
    }

    #[test]
    fn truncated_assignment_keeps_what_parsed() {
        let mut bytes = encode(0, &[("orders", &[0, 1])]);
        bytes.truncate(bytes.len() - 6);
        assert!(decode_assignment(&bytes).is_empty());
        assert!(decode_assignment(&[]).is_empty());
    }

    #[test]
    fn lag_math() {
        assert_eq!(lag(Some(10), 25), Some(15));
        assert_eq!(lag(Some(25), 25), Some(0));
        assert_eq!(lag(Some(30), 25), Some(0));
        assert_eq!(lag(None, 25), None);
    }
}
