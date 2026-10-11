use crate::kafka::{manager::ClientManager, settings::ConnectionSettings};

/// Long-lived backend state shared by all commands.
#[derive(Default)]
pub struct AppState {
    pub clients: ClientManager,
}

impl AppState {
    /// The connection commands act on. Until connection profiles exist this
    /// is the local development broker.
    pub fn active_connection(&self) -> ConnectionSettings {
        ConnectionSettings::local()
    }
}
