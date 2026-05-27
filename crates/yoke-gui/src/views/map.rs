use egui::{Color32, Pos2, Sense, Stroke, Vec2};

use crate::app::YokeApp;
use crate::stations::{FPS_STATIONS, VIEWBOX_H, VIEWBOX_W, binding_counts};

pub fn show(app: &mut YokeApp, ui: &mut egui::Ui) {
    let counts = app
        .open_profile()
        .and_then(|op| op.profile.sub_profiles.get(app.selected_subprofile()))
        .map(binding_counts)
        .unwrap_or_default();

    let palette = *app.palette();
    let desired = Vec2::new(ui.available_width().min(520.0), 0.0);
    let (rect, _resp) = ui.allocate_exact_size(
        Vec2::new(desired.x, desired.x * (VIEWBOX_H / VIEWBOX_W)),
        Sense::hover(),
    );
    let painter = ui.painter_at(rect);
    let sx = rect.width() / VIEWBOX_W;
    let sy = rect.height() / VIEWBOX_H;
    let to_screen =
        |x: f32, y: f32| Pos2::new(x.mul_add(sx, rect.left()), y.mul_add(sy, rect.top()));

    let mut clicked: Option<&'static str> = None;
    for st in FPS_STATIONS {
        let center = to_screen(st.x, st.y);
        let radius = if st.id == "joystick" { 16.0 } else { 11.0 };
        let id = ui.id().with(st.id);
        let resp = ui.interact(
            egui::Rect::from_center_size(center, Vec2::splat(radius * 2.0)),
            id,
            Sense::click(),
        );
        let selected = app.selected_station() == Some(st.id);
        let count = counts.get(st.id).copied().unwrap_or(0);

        let fill = if selected {
            palette.accent
        } else if count > 0 {
            palette.bg_binding
        } else {
            Color32::from_gray(60)
        };
        let stroke_color = if selected {
            palette.accent
        } else {
            palette.line
        };
        painter.circle(center, radius, fill, Stroke::new(1.5, stroke_color));
        if selected {
            painter.circle_stroke(center, radius + 3.0, Stroke::new(1.0, palette.accent));
        }
        if count > 0 && !selected {
            painter.text(
                center,
                egui::Align2::CENTER_CENTER,
                count.to_string(),
                egui::FontId::monospace(12.0),
                palette.ink_1,
            );
        }
        painter.text(
            center + Vec2::new(0.0, radius + 10.0),
            egui::Align2::CENTER_CENTER,
            st.label,
            egui::FontId::proportional(11.0),
            if selected {
                palette.accent
            } else {
                palette.ink_2
            },
        );
        if resp.clicked() {
            clicked = Some(st.id);
        }
        resp.on_hover_text(st.label);
    }

    if let Some(id) = clicked {
        // Toggle: clicking the selected station clears the filter.
        let next = if app.selected_station() == Some(id) {
            None
        } else {
            Some(id)
        };
        app.set_selected_station(next);
    }
}
