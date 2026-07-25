//! Custom window chrome (design `.win-chrome`).

use egui::{
    Align, Align2, Color32, CornerRadius, FontId, Layout, PointerButton, Rect, Sense, Stroke,
    StrokeKind, UiBuilder, ViewportCommand, pos2, vec2,
};

use crate::theme::{BG_2, BG_3, BG_4, INK_1, INK_2, LINE, R_FULL};

pub const HEIGHT: f32 = 40.0;

const CLOSE_HOVER: Color32 = Color32::from_rgb(0xC4, 0x2B, 0x1C);

/// Panel frame for the chrome strip. No margins: the caption buttons must sit
/// flush against the window edge.
pub fn frame() -> egui::Frame {
    egui::Frame::new().fill(BG_3)
}

pub fn strip(ui: &mut egui::Ui, status_text: &str, dot_color: Color32) {
    let rect = ui.max_rect();
    // Registered before the pill and buttons so those, added later, win
    // hit-testing over the drag area (the upstream custom_window_frame pattern).
    let drag = ui.interact(
        rect,
        ui.id().with("win_chrome_drag"),
        Sense::click_and_drag(),
    );

    // Centered on the full strip so the title sits at the window's center, not
    // the center of the space left over by the pill.
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        "Yoke",
        crate::theme::semibold(13.0),
        INK_1,
    );

    ui.scope_builder(
        UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::right_to_left(Align::Center)),
        |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            if cfg!(target_os = "windows") {
                caption_buttons(ui);
            }
            ui.add_space(12.0);
            conn_pill(ui, status_text, dot_color);
        },
    );

    if cfg!(not(target_arch = "wasm32")) {
        if drag.drag_started_by(PointerButton::Primary) {
            ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
        }
        if drag.double_clicked() {
            toggle_maximized(ui.ctx());
        }
    }
}

fn toggle_maximized(ctx: &egui::Context) {
    let maximized = ctx.input(|i| i.viewport().maximized) == Some(true);
    ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
}

/// Connection pill (design `.conn`). Painted directly so the layout is
/// direction-independent (the strip lays out right-to-left).
fn conn_pill(ui: &mut egui::Ui, text: &str, dot_color: Color32) {
    const PAD_X: f32 = 10.0;
    const PAD_Y: f32 = 4.0;
    const DOT: f32 = 7.0;
    const GAP: f32 = 6.0;
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), FontId::monospace(11.0), INK_2);
    let size = vec2(
        PAD_X + DOT + GAP + galley.size().x + PAD_X,
        PAD_Y.mul_add(2.0, galley.size().y),
    );
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(R_FULL),
        BG_2,
        Stroke::new(1.0_f32, LINE),
        StrokeKind::Inside,
    );
    painter.circle_filled(
        pos2(rect.left() + PAD_X + DOT / 2.0, rect.center().y),
        DOT / 2.0,
        dot_color,
    );
    painter.galley(
        pos2(
            rect.left() + PAD_X + DOT + GAP,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        INK_2,
    );
}

#[derive(Clone, Copy)]
enum Caption {
    Minimize,
    Maximize,
    Restore,
    Close,
}

// Rendered right-to-left, so close comes first (rightmost, flush to the edge).
fn caption_buttons(ui: &mut egui::Ui) {
    if caption_button(ui, Caption::Close).clicked() {
        ui.ctx().send_viewport_cmd(ViewportCommand::Close);
    }
    let maximized = ui.input(|i| i.viewport().maximized) == Some(true);
    let glyph = if maximized {
        Caption::Restore
    } else {
        Caption::Maximize
    };
    if caption_button(ui, glyph).clicked() {
        ui.ctx()
            .send_viewport_cmd(ViewportCommand::Maximized(!maximized));
    }
    if caption_button(ui, Caption::Minimize).clicked() {
        ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true));
    }
}

fn caption_button(ui: &mut egui::Ui, glyph: Caption) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(46.0, HEIGHT), Sense::click());
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let is_close = matches!(glyph, Caption::Close);
    let hovered = response.hovered();
    let painter = ui.painter();
    if hovered {
        let fill = if is_close { CLOSE_HOVER } else { BG_4 };
        painter.rect_filled(rect, CornerRadius::ZERO, fill);
    }
    let ink = if is_close && hovered {
        Color32::WHITE
    } else {
        INK_2
    };
    let stroke = Stroke::new(1.0_f32, ink);
    let c = rect.center();
    match glyph {
        Caption::Minimize => {
            painter.line_segment([c - vec2(5.0, 0.0), c + vec2(5.0, 0.0)], stroke);
        }
        Caption::Maximize => {
            painter.rect_stroke(
                Rect::from_center_size(c, vec2(10.0, 10.0)),
                CornerRadius::ZERO,
                stroke,
                StrokeKind::Inside,
            );
        }
        Caption::Restore => {
            for offset in [vec2(1.5, -1.5), vec2(-1.5, 1.5)] {
                painter.rect_stroke(
                    Rect::from_center_size(c + offset, vec2(8.0, 8.0)),
                    CornerRadius::ZERO,
                    stroke,
                    StrokeKind::Inside,
                );
            }
        }
        Caption::Close => {
            let b = Rect::from_center_size(c, vec2(10.0, 10.0));
            painter.line_segment([b.left_top(), b.right_bottom()], stroke);
            painter.line_segment([b.left_bottom(), b.right_top()], stroke);
        }
    }
    response
}

/// Invisible edge/corner resize zones for the undecorated Windows window
/// (winit gives an undecorated window no native resize borders). Call after
/// all panels so the zones, added last, win hit-testing along the edges.
pub fn edge_resize(ui: &egui::Ui) {
    use egui::CursorIcon as C;
    use egui::viewport::ResizeDirection as D;
    const EDGE: f32 = 4.0;
    const CORNER: f32 = 8.0;
    if !cfg!(target_os = "windows") {
        return;
    }
    if ui.input(|i| i.viewport().maximized) == Some(true) {
        return;
    }
    let r = ui.ctx().content_rect();
    let corner = vec2(CORNER, CORNER);
    // Edges first: the corner zones added after them win where they overlap.
    let zones: [(Rect, D, C); 8] = [
        (
            Rect::from_min_max(r.min, pos2(r.max.x, r.min.y + EDGE)),
            D::North,
            C::ResizeNorth,
        ),
        (
            Rect::from_min_max(pos2(r.min.x, r.max.y - EDGE), r.max),
            D::South,
            C::ResizeSouth,
        ),
        (
            Rect::from_min_max(r.min, pos2(r.min.x + EDGE, r.max.y)),
            D::West,
            C::ResizeWest,
        ),
        (
            Rect::from_min_max(pos2(r.max.x - EDGE, r.min.y), r.max),
            D::East,
            C::ResizeEast,
        ),
        (
            Rect::from_min_size(r.min, corner),
            D::NorthWest,
            C::ResizeNorthWest,
        ),
        (
            Rect::from_min_size(pos2(r.max.x - CORNER, r.min.y), corner),
            D::NorthEast,
            C::ResizeNorthEast,
        ),
        (
            Rect::from_min_size(pos2(r.min.x, r.max.y - CORNER), corner),
            D::SouthWest,
            C::ResizeSouthWest,
        ),
        (
            Rect::from_min_size(r.max - corner, corner),
            D::SouthEast,
            C::ResizeSouthEast,
        ),
    ];
    for (i, (zone, dir, cursor)) in zones.into_iter().enumerate() {
        let response = ui.interact(zone, ui.id().with(("win_edge_resize", i)), Sense::drag());
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(cursor);
        }
        if response.drag_started_by(PointerButton::Primary) {
            ui.ctx()
                .send_viewport_cmd(ViewportCommand::BeginResize(dir));
        }
    }
}
