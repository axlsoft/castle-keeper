
#[tauri::command]
pub fn greet(name: &str) -> Result<String, String> {
    if name.trim().is_empty() {
        Err("Name cannot be empty".into())
    } else {
        Ok(format!("Hello, {}! You've been greeted from Rust!", name))
    }
}