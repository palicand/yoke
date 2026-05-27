// uniform view signature; real impl in a later task mutates app
#[allow(clippy::needless_pass_by_ref_mut)]
pub fn show(_app: &mut crate::app::YokeApp, ui: &mut egui::Ui) {
    ui.label("library");
}
