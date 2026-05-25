pub mod backend;

use std::sync::Arc;

use leptos::prelude::*;
use wasm_bindgen::prelude::*;

use backend::{Backend, mock::MockBackend};

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();
    leptos::mount::mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    let backend: Arc<dyn Backend> =
        Arc::new(MockBackend::new().expect("mock fixture should parse"));
    provide_context(backend);
    view! { <div class="qs-app">"Yoke UI (mock backend)"</div> }
}
