pub mod backend;
pub mod components;
pub mod effects;
pub mod state;
pub mod views;

use std::sync::Arc;

use leptos::prelude::*;

use backend::{Backend, mock::MockBackend};
use state::AppState;

#[cfg(target_arch = "wasm32")]
fn make_backend() -> Arc<dyn Backend> {
    if tauri_sys::core::is_tauri() {
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
pub fn App() -> impl IntoView {
    let backend: Arc<dyn Backend> = make_backend();
    let state = AppState::new(backend);
    effects::spawn_volume_subscription(&state);
    effects::spawn_community_fetch(&state);
    let open = state.open_profile;
    provide_context(state);
    view! {
        <components::app_shell::AppShell>
            <Show
                when=move || open.get().is_none()
                fallback=|| view! { <views::editor::EditorView/> }
            >
                <views::library::LibraryView/>
            </Show>
        </components::app_shell::AppShell>
    }
}
