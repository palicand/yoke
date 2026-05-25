// Bin is wasm-only meaningful: trunk builds this bin into the WASM module that
// boots the Leptos app. The host build exists so `cargo build --workspace`
// keeps a uniform target list — it does nothing.

#[cfg(target_arch = "wasm32")]
fn main() {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();
    leptos::mount::mount_to_body(yoke_ui::App);
}

#[cfg(not(target_arch = "wasm32"))]
const fn main() {}
