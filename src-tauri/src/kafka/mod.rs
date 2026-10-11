//! Kafka access: connection settings, the shared client manager, and the
//! operations commands build on.

pub mod connection;
pub mod manager;
pub mod produce;
pub mod settings;
pub mod topics;

use std::time::Duration;

use crate::error::{AppError, AppResult};

/// Extra time allowed on top of a librdkafka timeout before we give up on
/// the blocking call ourselves, so librdkafka normally reports first.
const BLOCKING_GRACE: Duration = Duration::from_millis(500);

/// Runs a blocking librdkafka call on Tokio's blocking pool so it never
/// stalls the async workers that serve other commands.
pub async fn run_blocking<T, F>(operation: &str, timeout: Duration, f: F) -> AppResult<T>
where
    F: FnOnce() -> AppResult<T> + Send + 'static,
    T: Send + 'static,
{
    let task = tokio::task::spawn_blocking(f);
    match tokio::time::timeout(timeout + BLOCKING_GRACE, task).await {
        Ok(joined) => joined?,
        Err(_) => Err(AppError::timeout(operation, timeout)),
    }
}
