#[tauri::command]
fn greet(name: &str) -> Result<String, String> {
    if name.trim().is_empty() {
        Err("Name cannot be empty".into())
    } else {
        Ok(format!("Hello, {}! You've been greeted from Rust!", name))
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
