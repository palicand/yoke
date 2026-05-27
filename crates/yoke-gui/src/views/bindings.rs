// uniform view signature; real impl in a later task mutates app
#[allow(clippy::needless_pass_by_ref_mut)]
#[allow(clippy::missing_const_for_fn)]
pub fn show(_app: &mut crate::app::YokeApp, _ui: &mut egui::Ui) {}
