use crate::app::YokeApp;
use crate::data::AppCommand;
use crate::state::CommunityLoad;

// uniform view signature; this view is read-only over app
#[allow(clippy::needless_pass_by_ref_mut)]
pub fn show(app: &mut YokeApp, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            ui.heading("Library");
            ui.add_space(8.0);

            ui.label(egui::RichText::new("QuadStick profiles").strong());
            let device_profiles = app.device_profiles().to_vec();
            if device_profiles.is_empty() {
                ui.label(
                    egui::RichText::new("No device profiles (volume not mounted).")
                        .color(app.palette().ink_3),
                );
            } else {
                for entry in &device_profiles {
                    if ui.button(&entry.label).clicked() {
                        app.open_device_profile(entry.name.clone(), entry.label.clone());
                    }
                }
            }

            // The browser build has no native file dialog.
            #[cfg(not(target_arch = "wasm32"))]
            {
                ui.add_space(12.0);
                if ui.button("Open CSV file...").clicked() {
                    app.open_file_dialog();
                }
            }

            ui.add_space(16.0);
            ui.label(egui::RichText::new("Community profiles").strong());
            match app.community().clone() {
                CommunityLoad::Loading => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Loading...");
                    });
                }
                CommunityLoad::Loaded(entries) => {
                    if entries.is_empty() {
                        ui.label(
                            egui::RichText::new("No community entries.").color(app.palette().ink_3),
                        );
                    }
                    for entry in entries {
                        let name = community_name(&entry);
                        if ui.button(&name).clicked() {
                            app.open_community(entry.clone(), name);
                        }
                    }
                }
                CommunityLoad::Failed(msg) => {
                    ui.colored_label(app.palette().system, format!("Failed: {msg}"));
                    if ui.button("Retry").clicked() {
                        app.send(AppCommand::ListCommunity);
                    }
                }
                CommunityLoad::Disabled => {
                    ui.label(
                        egui::RichText::new("Community profiles unavailable.")
                            .color(app.palette().ink_3),
                    );
                }
            }
        });
}

#[cfg(not(target_arch = "wasm32"))]
fn community_name(entry: &yoke_index::IndexEntry) -> String {
    entry.name.clone()
}
#[cfg(target_arch = "wasm32")]
fn community_name(entry: &crate::data::mock::MockCommunityEntry) -> String {
    entry.name.clone()
}
