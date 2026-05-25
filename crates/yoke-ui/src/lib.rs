pub mod backend;
pub mod effects;
pub mod state;

use std::sync::Arc;

use leptos::prelude::*;
use wasm_bindgen::prelude::*;

use backend::{Backend, mock::MockBackend};
use state::AppState;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();
    leptos::mount::mount_to_body(App);
}

#[cfg(target_arch = "wasm32")]
fn detect_tauri() -> bool {
    tauri_sys::core::is_tauri()
}

#[cfg(not(target_arch = "wasm32"))]
const fn detect_tauri() -> bool {
    false
}

#[cfg(target_arch = "wasm32")]
fn make_backend() -> Arc<dyn Backend> {
    if detect_tauri() {
        Arc::new(backend::tauri::TauriBackend::new())
    } else {
        Arc::new(MockBackend::new().expect("mock fixture should parse"))
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn make_backend() -> Arc<dyn Backend> {
    Arc::new(MockBackend::new().expect("mock fixture should parse"))
}

#[component]
fn App() -> impl IntoView {
    let backend: Arc<dyn Backend> = make_backend();
    let state = AppState::new(backend);
    effects::spawn_volume_subscription(&state);
    effects::spawn_community_fetch(&state);
    provide_context(state);
    let mode = if detect_tauri() { "tauri" } else { "mock" };
    view! { <div class="qs-app">{format!("Yoke UI ({mode} backend)")}</div> }
}
