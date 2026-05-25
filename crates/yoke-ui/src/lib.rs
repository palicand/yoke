pub mod backend;

use leptos::prelude::*;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();
    leptos::mount::mount_to_body(|| view! { <App/> });
}

#[component]
fn App() -> impl IntoView {
    view! { <div class="qs-app">"Yoke UI booting..."</div> }
}
