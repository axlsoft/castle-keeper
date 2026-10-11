//! Structured logging: stderr in development, a daily rolling file in the
//! app's log directory always. librdkafka logs arrive through rdkafka's
//! `tracing` feature.

use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

const DEFAULT_FILTER: &str = "info,castle_keeper_lib=debug,librdkafka=info,rdkafka=info";

/// Installs the global subscriber. Keep the returned guard alive for the
/// lifetime of the app so buffered file output is flushed on exit.
pub fn init(log_dir: &Path) -> Option<WorkerGuard> {
    let filter = EnvFilter::try_from_env("CASTLE_KEEPER_LOG")
        .unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));

    let (file_writer, guard) = match tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("castle-keeper")
        .filename_suffix("log")
        .max_log_files(14)
        .build(log_dir)
    {
        Ok(appender) => {
            let (writer, guard) = tracing_appender::non_blocking(appender);
            (Some(writer), Some(guard))
        }
        Err(error) => {
            eprintln!("file logging disabled: {error}");
            (None, None)
        }
    };

    let file_layer = file_writer.map(|writer| fmt::layer().with_ansi(false).with_writer(writer));
    let stderr_layer = cfg!(debug_assertions).then(|| fmt::layer().with_writer(std::io::stderr));

    let result = tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stderr_layer)
        .try_init();
    if let Err(error) = result {
        eprintln!("logging already initialized: {error}");
    }
    guard
}
