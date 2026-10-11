//! Batched offset lookups. Watermarks use ListOffsets with the special
//! "earliest" (-2) and "latest" (-1) timestamps, which librdkafka groups
//! into one request per broker instead of one round trip per partition.

use std::{collections::HashMap, time::Duration};

use rdkafka::{
    Offset, TopicPartitionList,
    consumer::{BaseConsumer, Consumer},
};

use crate::error::AppResult;

pub type PartitionKey = (String, i32);

// rdkafka's End/Beginning carry the raw values -1/-2, which ListOffsets
// reads as the LATEST/EARLIEST timestamps.
const LATEST: Offset = Offset::End;
const EARLIEST: Offset = Offset::Beginning;

/// Low and high watermarks for every requested partition.
pub fn watermarks(
    consumer: &BaseConsumer,
    partitions: &[PartitionKey],
    timeout: Duration,
) -> AppResult<HashMap<PartitionKey, (i64, i64)>> {
    if partitions.is_empty() {
        return Ok(HashMap::new());
    }
    let lows = list_offsets(consumer, partitions, EARLIEST, timeout)?;
    let highs = list_offsets(consumer, partitions, LATEST, timeout)?;
    let mut out = HashMap::with_capacity(partitions.len());
    for key in partitions {
        let (low, high) = match (lows.get(key), highs.get(key)) {
            (Some(&low), Some(&high)) => (low, high),
            // Fall back to a direct query for anything the batch missed.
            _ => consumer.fetch_watermarks(&key.0, key.1, timeout)?,
        };
        out.insert(key.clone(), (low, high));
    }
    Ok(out)
}

fn list_offsets(
    consumer: &BaseConsumer,
    partitions: &[PartitionKey],
    timestamp: Offset,
    timeout: Duration,
) -> AppResult<HashMap<PartitionKey, i64>> {
    let mut tpl = TopicPartitionList::with_capacity(partitions.len());
    for (topic, partition) in partitions {
        tpl.add_partition_offset(topic, *partition, timestamp)?;
    }
    let result = consumer.offsets_for_times(tpl, timeout)?;
    Ok(result
        .elements()
        .into_iter()
        .filter(|e| e.error().is_ok())
        .filter_map(|e| match e.offset() {
            Offset::Offset(offset) if offset >= 0 => {
                Some(((e.topic().to_string(), e.partition()), offset))
            }
            _ => None,
        })
        .collect())
}

/// Committed offsets of `consumer`'s group for the given partitions. The
/// consumer must have `group.id` set; it never joins the group.
pub fn committed(
    consumer: &BaseConsumer,
    partitions: &[PartitionKey],
    timeout: Duration,
) -> AppResult<HashMap<PartitionKey, i64>> {
    if partitions.is_empty() {
        return Ok(HashMap::new());
    }
    let mut tpl = TopicPartitionList::with_capacity(partitions.len());
    for (topic, partition) in partitions {
        tpl.add_partition(topic, *partition);
    }
    let result = consumer.committed_offsets(tpl, timeout)?;
    Ok(result
        .elements()
        .into_iter()
        .filter_map(|e| match e.offset() {
            Offset::Offset(offset) if offset >= 0 => {
                Some(((e.topic().to_string(), e.partition()), offset))
            }
            _ => None,
        })
        .collect())
}
