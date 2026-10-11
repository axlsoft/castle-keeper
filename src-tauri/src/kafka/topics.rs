use std::sync::Arc;

use rdkafka::consumer::Consumer;

use super::{manager::ClusterClients, run_blocking};
use crate::error::AppResult;

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
