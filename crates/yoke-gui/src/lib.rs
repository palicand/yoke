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

    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;

    // The `Result<(), JsValue>` return type is mandated by the
    // `#[wasm_bindgen(start)]` contract; the Ok value is never the point.
    #[wasm_bindgen(start)]
    #[allow(clippy::unnecessary_wraps)]
    pub fn start() -> Result<(), JsValue> {
        console_error_panic_hook::set_once();
        tracing_wasm::set_as_global_default();

        let web_options = eframe::WebOptions::default();
        wasm_bindgen_futures::spawn_local(async {
            let document = web_sys::window().unwrap().document().unwrap();
            let canvas = document
                .get_element_by_id("yoke_canvas")
                .unwrap()
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .unwrap();
            eframe::WebRunner::new()
                .start(
                    canvas,
                    web_options,
                    Box::new(|cc| {
                        crate::theme::install_fonts(&cc.egui_ctx);
                        crate::theme::apply(&cc.egui_ctx);
                        let data = Rc::new(crate::data::mock::MockDataSource::new());
                        let worker = crate::worker::spawn(data);
                        Ok(Box::new(crate::app::YokeApp::new(worker)))
                    }),
                )
                .await
                .expect("failed to start eframe");
        });
        Ok(())
    }
}
