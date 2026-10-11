#![deny(clippy::unwrap_used, clippy::expect_used)]

use tauri::Manager;

mod commands;
pub mod error;
pub mod kafka;
mod logging;
pub mod profiles;
pub mod state;

/// Keeps the non-blocking log writer alive until the app exits.
struct LogGuard(#[allow(dead_code)] Option<tracing_appender::non_blocking::WorkerGuard>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let specta = commands::specta_builder();
    // Keep the frontend bindings in sync while developing.
    #[cfg(debug_assertions)]
    if let Err(error) = specta.export(commands::typescript(), commands::BINDINGS_PATH) {
        eprintln!("could not export TypeScript bindings: {error}");
    }

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
            app.manage(state::AppState::load(&app.path().app_config_dir()?));
            tracing::info!(version = env!("CARGO_PKG_VERSION"), log_dir = %log_dir.display(), "Castle Keeper starting");
            Ok(())
        })
        .invoke_handler(specta.invoke_handler())
        .run(tauri::generate_context!());

    if let Err(error) = result {
        tracing::error!(%error, "error while running tauri application");
        eprintln!("error while running tauri application: {error}");
        std::process::exit(1);
    }
}
