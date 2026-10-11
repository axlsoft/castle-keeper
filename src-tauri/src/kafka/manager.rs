//! Pooled Kafka clients, one set per connection, created lazily and reused
//! across commands.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, RwLock,
    },
    time::Duration,
};

use rdkafka::{
    admin::AdminClient, client::DefaultClientContext, consumer::BaseConsumer,
    producer::FutureProducer,
};

use super::settings::ConnectionSettings;
use crate::error::{AppError, AppResult};

pub type Admin = AdminClient<DefaultClientContext>;

/// Clients for one cluster connection.
pub struct ClusterClients {
    settings: ConnectionSettings,
    /// A consumer without a group id: never joins a group, used for
    /// metadata and offset lookups.
    metadata: Arc<BaseConsumer>,
    admin: Admin,
    producer: Mutex<Option<FutureProducer>>,
    /// Consumers with a `group.id`, used only to read that group's committed
    /// offsets. They never subscribe or commit, so they don't join the group.
    group_consumers: Mutex<HashMap<String, Arc<BaseConsumer>>>,
    stop_polling: Arc<AtomicBool>,
}

impl ClusterClients {
    fn connect(settings: ConnectionSettings) -> AppResult<Self> {
        let metadata: Arc<BaseConsumer> = Arc::new(settings.client_config().create()?);
        let admin: Admin = settings.client_config().create()?;
        let stop_polling = Arc::new(AtomicBool::new(false));
        spawn_event_loop(&settings.id, metadata.clone(), stop_polling.clone());
        Ok(Self {
            settings,
            metadata,
            admin,
            producer: Mutex::new(None),
            group_consumers: Mutex::new(HashMap::new()),
            stop_polling,
        })
    }

    pub fn settings(&self) -> &ConnectionSettings {
        &self.settings
    }

    pub fn metadata_client(&self) -> &BaseConsumer {
        &self.metadata
    }

    pub fn admin(&self) -> &Admin {
        &self.admin
    }

    /// The shared producer, created on first use.
    pub fn producer(&self) -> AppResult<FutureProducer> {
        let mut slot = self
            .producer
            .lock()
            .map_err(|_| AppError::internal("producer lock poisoned"))?;
        if let Some(producer) = slot.as_ref() {
            return Ok(producer.clone());
        }
        let producer: FutureProducer = self
            .settings
            .client_config()
            .set(
                "message.timeout.ms",
                self.settings.delivery_timeout.as_millis().to_string(),
            )
            .create()?;
        *slot = Some(producer.clone());
        Ok(producer)
    }
}

impl ClusterClients {
    /// A consumer for reading `group_id`'s committed offsets, cached.
    pub fn group_consumer(&self, group_id: &str) -> AppResult<Arc<BaseConsumer>> {
        let mut consumers = self
            .group_consumers
            .lock()
            .map_err(|_| AppError::internal("group consumer lock poisoned"))?;
        if let Some(existing) = consumers.get(group_id) {
            return Ok(existing.clone());
        }
        let consumer: Arc<BaseConsumer> = Arc::new(
            self.settings
                .client_config()
                .set("group.id", group_id)
                .set("enable.auto.commit", "false")
                .set("enable.auto.offset.store", "false")
                .create()?,
        );
        consumers.insert(group_id.to_string(), consumer.clone());
        Ok(consumer)
    }
}

impl Drop for ClusterClients {
    fn drop(&mut self) {
        self.stop_polling.store(true, Ordering::Relaxed);
    }
}

/// Serves the metadata client's main queue so librdkafka log and error
/// events are delivered (to tracing) instead of piling up.
fn spawn_event_loop(id: &str, consumer: Arc<BaseConsumer>, stop: Arc<AtomicBool>) {
    let name = format!("kafka-events-{id}");
    let spawned = std::thread::Builder::new().name(name).spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            if let Some(Err(error)) = consumer.poll(Duration::from_millis(250)) {
                tracing::debug!(%error, "client event");
            }
        }
    });
    if let Err(error) = spawned {
        tracing::warn!(%error, "could not start the Kafka event loop");
    }
}

#[derive(Default)]
pub struct ClientManager {
    clients: RwLock<HashMap<String, Arc<ClusterClients>>>,
}

impl ClientManager {
    /// Returns cached clients for these settings, replacing them if the
    /// settings changed since they were created.
    pub fn clients(&self, settings: &ConnectionSettings) -> AppResult<Arc<ClusterClients>> {
        {
            let clients = self
                .clients
                .read()
                .map_err(|_| AppError::internal("client cache lock poisoned"))?;
            if let Some(existing) = clients.get(&settings.id) {
                if existing.settings == *settings {
                    return Ok(existing.clone());
                }
            }
        }
        let fresh = Arc::new(ClusterClients::connect(settings.clone())?);
        let mut clients = self
            .clients
            .write()
            .map_err(|_| AppError::internal("client cache lock poisoned"))?;
        clients.insert(settings.id.clone(), fresh.clone());
        tracing::debug!(connection = %settings.id, bootstrap = %settings.bootstrap, "created Kafka clients");
        Ok(fresh)
    }

    /// Drops the clients for a connection, e.g. after its profile changed.
    pub fn evict(&self, id: &str) {
        if let Ok(mut clients) = self.clients.write() {
            clients.remove(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reuses_clients_until_settings_change() {
        let manager = ClientManager::default();
        let settings = ConnectionSettings::new("a", "127.0.0.1:1");
        let first = manager.clients(&settings).unwrap();
        let again = manager.clients(&settings).unwrap();
        assert!(Arc::ptr_eq(&first, &again));

        let mut changed = settings.clone();
        changed.bootstrap = "127.0.0.1:2".into();
        let replaced = manager.clients(&changed).unwrap();
        assert!(!Arc::ptr_eq(&first, &replaced));

        manager.evict("a");
        let recreated = manager.clients(&changed).unwrap();
        assert!(!Arc::ptr_eq(&replaced, &recreated));
    }
}
