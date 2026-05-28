pub mod app;
pub mod data;
pub mod state;
pub mod stations;
pub mod theme;
pub mod views;
pub mod worker;

#[cfg(target_arch = "wasm32")]
mod wasm_entry {
    use std::rc::Rc;

    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;

    // The `Result<(), JsValue>` return type is mandated by the
    // `#[wasm_bindgen(start)]` contract; the Ok value is never the point.
    #[wasm_bindgen(start)]
    #[allow(clippy::unnecessary_wraps)]
    pub fn start() -> Result<(), JsValue> {
        console_error_panic_hook::set_once();
        tracing_wasm::set_as_global_default();

        let web_options = eframe::WebOptions::default();
        wasm_bindgen_futures::spawn_local(async {
            // An external embedder may load the wasm artifact in a page without
            // `#yoke_canvas`; log and bail rather than panicking into a blank
            // page after `start()` has already returned Ok.
            let Some(window) = web_sys::window() else {
                tracing::error!("window unavailable; cannot start yoke");
                return;
            };
            let Some(document) = window.document() else {
                tracing::error!("document unavailable; cannot start yoke");
                return;
            };
            let Some(canvas_el) = document.get_element_by_id("yoke_canvas") else {
                tracing::error!("missing `#yoke_canvas` element; cannot start yoke");
                return;
            };
            let Ok(canvas) = canvas_el.dyn_into::<web_sys::HtmlCanvasElement>() else {
                tracing::error!("`#yoke_canvas` is not a canvas element; cannot start yoke");
                return;
            };
            let start_result = eframe::WebRunner::new()
                .start(
                    canvas,
                    web_options,
                    Box::new(|cc| {
                        crate::theme::install_fonts(&cc.egui_ctx);
                        crate::theme::apply(&cc.egui_ctx);
                        let data = Rc::new(crate::data::mock::MockDataSource::new());
                        let worker = crate::worker::spawn(data);
                        // Mock fixture always provides community entries.
                        Ok(Box::new(crate::app::YokeApp::new(worker, true)))
                    }),
                )
                .await;
            if let Err(err) = start_result {
                tracing::error!(?err, "failed to start eframe");
            }
        });
        Ok(())
    }
}
