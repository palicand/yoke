use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, Vec2};

use crate::app::YokeApp;
use crate::stations::{FPS_STATIONS, StationKind, VIEWBOX_H, VIEWBOX_W, binding_counts};

// Cluster label + padding (viewbox units), drawn as a dashed region box.
const REGIONS: &[(StationKind, &str, f32)] = &[
    (StationKind::Joystick, "JOYSTICK", 9.0),
    (StationKind::Mouthpiece, "MOUTHPIECE", 9.0),
    (StationKind::Lip, "LIP", 7.0),
    (StationKind::Side, "SIDE TUBE", 7.0),
];

pub fn show(app: &mut YokeApp, ui: &mut egui::Ui) {
    let counts = app
        .open_profile()
        .and_then(|op| op.profile.sub_profiles.get(app.selected_subprofile()))
        .map(binding_counts)
        .unwrap_or_default();

    let palette = *app.palette();
    let width = ui.available_width().min(560.0);
    let (rect, _resp) = ui.allocate_exact_size(
        Vec2::new(width, width * (VIEWBOX_H / VIEWBOX_W)),
        Sense::hover(),
    );
    let painter = ui.painter_at(rect);
    let scale = rect.width() / VIEWBOX_W;
    let to_screen =
        |x: f32, y: f32| Pos2::new(x.mul_add(scale, rect.left()), y.mul_add(scale, rect.top()));

    draw_grid(&painter, &to_screen, palette.ink_3);
    draw_regions(&painter, &to_screen, scale, palette.line, palette.ink_3);
    draw_mouthpiece_rail(&painter, &to_screen, palette.ink_3);

    let mut clicked: Option<&'static str> = None;
    for st in FPS_STATIONS {
        let center = to_screen(st.x, st.y);
        let r_units = match st.kind {
            StationKind::Joystick => 4.8,
            StationKind::Mouthpiece => 2.6,
            StationKind::Lip | StationKind::Side => 2.2,
        };
        let radius = r_units * scale;
        let selected = app.selected_station() == Some(st.id);
        let count = counts.get(st.id).copied().unwrap_or(0);

        let id = ui.id().with(st.id);
        let resp = ui.interact(
            Rect::from_center_size(center, Vec2::splat(radius * 2.0)),
            id,
            Sense::click(),
        );

        let fill = if selected {
            palette.accent
        } else if count > 0 {
            palette.bg_binding
        } else {
            Color32::from_white_alpha(20)
        };
        let stroke_color = if selected {
            palette.accent
        } else {
            palette.ink_2
        };
        painter.circle(center, radius, fill, Stroke::new(1.0, stroke_color));
        if selected {
            painter.circle_stroke(center, radius + 3.0, Stroke::new(1.0, palette.accent));
        }
        if count > 0 && !selected {
            painter.text(
                center,
                Align2::CENTER_CENTER,
                count.to_string(),
                FontId::monospace((2.6 * scale).min(15.0)),
                palette.ink_1,
            );
        }
        painter.text(
            center + Vec2::new(0.0, radius + 9.0),
            Align2::CENTER_CENTER,
            short_label(st.id),
            FontId::monospace((2.0 * scale).min(12.0)),
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

    painter.text(
        rect.left_bottom() + Vec2::new(4.0, -4.0),
        Align2::LEFT_BOTTOM,
        "QS · FPS · INPUT MAP",
        FontId::monospace(10.0),
        palette.ink_3,
    );

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

fn short_label(id: &str) -> &'static str {
    match id {
        "joystick" => "JOY",
        "mp_left" => "L",
        "mp_left_center" => "L·C",
        "mp_center" => "C",
        "mp_right_center" => "R·C",
        "mp_right" => "R",
        "lip" => "LIP",
        "side" => "SIDE",
        _ => "",
    }
}

fn draw_grid(painter: &egui::Painter, to_screen: &impl Fn(f32, f32) -> Pos2, ink_3: Color32) {
    let dot = ink_3.gamma_multiply(0.28);
    // 5-unit spacing across the 100x80 viewbox (21 x 17 dots).
    for i in 0..=20u8 {
        for j in 0..=16u8 {
            let p = to_screen(f32::from(i) * 5.0, f32::from(j) * 5.0);
            painter.circle_filled(p, 0.6, dot);
        }
    }
}

fn draw_regions(
    painter: &egui::Painter,
    to_screen: &impl Fn(f32, f32) -> Pos2,
    scale: f32,
    line: Color32,
    ink_3: Color32,
) {
    for &(kind, label, pad) in REGIONS {
        let pts: Vec<(f32, f32)> = FPS_STATIONS
            .iter()
            .filter(|s| s.kind == kind)
            .map(|s| (s.x, s.y))
            .collect();
        if pts.is_empty() {
            continue;
        }
        let min_x = pts.iter().map(|p| p.0).fold(f32::INFINITY, f32::min) - pad;
        let max_x = pts.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max) + pad;
        let min_y = pts.iter().map(|p| p.1).fold(f32::INFINITY, f32::min) - pad;
        let max_y = pts.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max) + pad;
        let r = Rect::from_min_max(to_screen(min_x, min_y), to_screen(max_x, max_y));
        painter.rect_filled(r, 4.0, Color32::from_white_alpha(4));
        dashed_rect(painter, r, Stroke::new(1.0, line));
        painter.text(
            r.left_top() + Vec2::new(0.0, -3.0),
            Align2::LEFT_BOTTOM,
            label,
            FontId::monospace((1.7 * scale).min(11.0)),
            ink_3,
        );
    }
}

fn draw_mouthpiece_rail(
    painter: &egui::Painter,
    to_screen: &impl Fn(f32, f32) -> Pos2,
    ink_3: Color32,
) {
    let mp: Vec<f32> = FPS_STATIONS
        .iter()
        .filter(|s| s.kind == StationKind::Mouthpiece)
        .map(|s| s.x)
        .collect();
    if mp.len() < 2 {
        return;
    }
    let y = FPS_STATIONS
        .iter()
        .find(|s| s.kind == StationKind::Mouthpiece)
        .map_or(48.0, |s| s.y);
    let left = mp.iter().copied().fold(f32::INFINITY, f32::min);
    let right = mp.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    painter.extend(Shape::dashed_line(
        &[to_screen(left, y), to_screen(right, y)],
        Stroke::new(1.0, ink_3.gamma_multiply(0.6)),
        3.0,
        3.0,
    ));
}

fn dashed_rect(painter: &egui::Painter, rect: Rect, stroke: Stroke) {
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
        rect.left_top(),
    ];
    for edge in corners.windows(2) {
        painter.extend(Shape::dashed_line(edge, stroke, 5.0, 4.0));
    }
}
