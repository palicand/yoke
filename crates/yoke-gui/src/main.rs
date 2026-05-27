#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    use std::sync::Arc;
    use yoke_gui::data::native::NativeDataSource;

    tracing_subscriber::fmt::init();

    // Volume provider: macOS impl, else a never-present fs backend fallback.
    let (provider, backend_error): (Arc<dyn yoke_volume::VolumeProvider>, Option<String>) = {
        #[cfg(target_os = "macos")]
        {
            match yoke_volume_macos::MacOsVolumeProvider::new() {
                Ok(p) => (Arc::new(p), None),
                Err(e) => (
                    Arc::new(yoke_volume::FsBackend::new(std::path::PathBuf::from("/Volumes/QUADSTICK"))),
                    Some(e.to_string()),
                ),
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            (
                Arc::new(yoke_volume::FsBackend::new(std::path::PathBuf::from("/Volumes/QUADSTICK"))),
                None,
            )
        }
    };

    let data = match NativeDataSource::new(provider) {
        Ok(d) => Arc::new(d),
        Err(e) => {
            eprintln!("fatal: {e}");
            std::process::exit(1);
        }
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Yoke")
            .with_inner_size([1100.0, 720.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Yoke",
        options,
        Box::new(move |cc| {
            yoke_gui::theme::apply(&cc.egui_ctx);
            let (worker, events) = yoke_gui::worker::spawn(data, cc.egui_ctx.clone());
            Ok(Box::new(yoke_gui::app::YokeApp::new(worker, events, backend_error)))
        }),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}
