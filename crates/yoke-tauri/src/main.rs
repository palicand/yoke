#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod volume_backend;
mod volume_watch;

use std::sync::Arc;
use yoke_volume::VolumeProvider;

pub struct AppState {
    pub volume: Arc<dyn VolumeProvider>,
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let volume = volume_backend::build_provider().expect("failed to initialize volume backend");
    let volume_for_setup = Arc::clone(&volume);

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState { volume })
        .invoke_handler(tauri::generate_handler![commands::volume::volume_state])
        .setup(move |app| {
            volume_watch::spawn(app.handle().clone(), &volume_for_setup);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
