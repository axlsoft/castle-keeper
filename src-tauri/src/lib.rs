#![deny(clippy::unwrap_used, clippy::expect_used)]

use commands::Pipe;
use tauri::Manager;

mod commands;
pub mod error;
mod logging;

/// Keeps the non-blocking log writer alive until the app exits.
struct LogGuard(#[allow(dead_code)] Option<tracing_appender::non_blocking::WorkerGuard>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .setup(|app| {
            let log_dir = app.path().app_log_dir()?;
            let guard = logging::init(&log_dir);
            app.manage(LogGuard(guard));
            tracing::info!(version = env!("CARGO_PKG_VERSION"), log_dir = %log_dir.display(), "Castle Keeper starting");
            Ok(())
        })
        .pipe(commands::register_commands)
        .run(tauri::generate_context!());

    if let Err(error) = result {
        tracing::error!(%error, "error while running tauri application");
        eprintln!("error while running tauri application: {error}");
        std::process::exit(1);
    }
}
