pub mod actions;
pub mod produce;
pub mod profiles;
pub mod topics;

#[cfg(any(test, debug_assertions))]
use specta_typescript::Typescript;
use tauri_specta::{collect_commands, Builder, ErrorHandlingMode};

/// The generated TypeScript bindings in the frontend source tree.
#[cfg(any(test, debug_assertions))]
pub const BINDINGS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/bindings.ts");

/// Every IPC command, with types exported to TypeScript.
pub fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            topics::list_topics,
            produce::produce_message,
            profiles::list_profiles,
            profiles::create_profile,
            profiles::update_profile,
            profiles::delete_profile,
            profiles::select_profile,
            profiles::test_connection,
            profiles::import_client_properties,
            actions::list_actions,
            actions::export_action_log,
            actions::unlock_profile,
            actions::lock_profile,
        ])
        .error_handling(ErrorHandlingMode::Throw)
        // Offsets are i64; Kafka offsets stay far below 2^53.
        .dangerously_cast_bigints_to_number()
}

#[cfg(any(test, debug_assertions))]
pub fn typescript() -> Typescript {
    Typescript::default().header("// Generated from the Rust command definitions. Do not edit.\n// Regenerate with `npm run bindings`.\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regenerates the bindings and fails if the committed file was stale.
    #[test]
    fn typescript_bindings_are_up_to_date() {
        let path = std::path::Path::new(BINDINGS_PATH);
        let before = std::fs::read_to_string(path).unwrap_or_default();
        specta_builder().export(typescript(), path).unwrap();
        let after = std::fs::read_to_string(path).unwrap();
        assert!(
            before == after,
            "src/lib/bindings.ts was out of date and has been regenerated; commit the update"
        );
    }
}
