use tauri::Wry;

pub mod cluster_admin;
pub mod publisher;
pub mod test_commands;

pub trait Pipe: Sized {
    fn pipe<F>(self, f: F) -> Self
    where
        F: FnOnce(Self) -> Self,
    {
        f(self)
    }
}

impl<T> Pipe for T {}

// Optionally, create a helper function to register commands.
pub fn register_commands(builder: tauri::Builder<Wry>) -> tauri::Builder<Wry> {
    builder.invoke_handler(tauri::generate_handler![
        cluster_admin::get_topics,
        publisher::send_message,
        test_commands::greet
    ])
}
