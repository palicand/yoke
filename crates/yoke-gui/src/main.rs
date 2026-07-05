#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    use std::sync::Arc;
    use yoke_gui::data::DataSource;
    use yoke_gui::data::native::NativeDataSource;

    tracing_subscriber::fmt::init();

    // Volume provider: native impl per platform, else a never-present fs
    // backend fallback.
    let (provider, backend_error): (Arc<dyn yoke_volume::VolumeProvider>, Option<String>) = {
        #[cfg(target_os = "macos")]
        {
            match yoke_volume_macos::MacOsVolumeProvider::new() {
                Ok(p) => (Arc::new(p), None),
                Err(e) => (
                    Arc::new(yoke_volume::FsBackend::new(std::path::PathBuf::from(
                        "/Volumes/QUADSTICK",
                    ))),
                    Some(e.to_string()),
                ),
            }
        }
        #[cfg(target_os = "windows")]
        {
            match yoke_volume_windows::WindowsVolumeProvider::new() {
                Ok(p) => (Arc::new(p), None),
                Err(e) => (
                    Arc::new(yoke_volume::FsBackend::new(std::path::PathBuf::from(
                        "/Volumes/QUADSTICK",
                    ))),
                    Some(e.to_string()),
                ),
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            (
                Arc::new(yoke_volume::FsBackend::new(std::path::PathBuf::from(
                    "/Volumes/QUADSTICK",
                ))),
                None,
            )
        }
    };

    let data = match NativeDataSource::new(provider) {
        Ok(d) => Arc::new(d),
        Err(e) => {
            tracing::error!(error = %e, "failed to initialize NativeDataSource");
            std::process::exit(1);
        }
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Yoke")
            .with_inner_size([1100.0, 720.0]),
        // AutoVsync (Metal Fifo) makes get_current_texture's nextDrawable() block
        // on the vsync-paced drawable queue, which freezes the window during
        // macOS's synchronous live-resize loop. AutoNoVsync maps to Metal
        // Immediate (displaySyncEnabled = NO) so acquiring a drawable never waits
        // for vsync. The app is reactive, so there is no idle GPU cost.
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            present_mode: eframe::wgpu::PresentMode::AutoNoVsync,
            ..Default::default()
        },
        ..Default::default()
    };

    eframe::run_native(
        "Yoke",
        options,
        Box::new(move |cc| {
            yoke_gui::theme::install_fonts(&cc.egui_ctx);
            yoke_gui::theme::apply(&cc.egui_ctx);
            let community_available = data.is_community_available();
            let (worker, events) = yoke_gui::worker::spawn(&data, &cc.egui_ctx);
            Ok(Box::new(yoke_gui::app::YokeApp::new(
                worker,
                events,
                backend_error,
                community_available,
            )))
        }),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}
