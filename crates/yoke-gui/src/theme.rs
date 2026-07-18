use egui::Color32;

/// Console (dark) accent + category colors. Hex values are the design-handoff
/// `[data-theme="console"]` tokens (oklch accents converted to sRGB).
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub accent: Color32,
    pub accent_2: Color32,
    pub keyboard: Color32,
    pub mouse: Color32,
    pub gamepad: Color32,
    pub dpad: Color32,
    pub joystick: Color32,
    pub system: Color32,
    pub bg_binding: Color32,
    pub ink_1: Color32,
    pub ink_2: Color32,
    pub ink_3: Color32,
    pub line: Color32,
}

// Mask off each byte before casting so the shift-and-cast is truncation-free.
const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb(
        ((hex >> 16) & 0xFF) as u8,
        ((hex >> 8) & 0xFF) as u8,
        (hex & 0xFF) as u8,
    )
}

impl Palette {
    #[must_use]
    pub const fn console() -> Self {
        Self {
            accent: rgb(0x84_C4_85),
            accent_2: rgb(0x21_33_21),
            keyboard: rgb(0x6F_BE_FF),
            mouse: rgb(0xF9_B6_4F),
            gamepad: rgb(0x75_D8_7A),
            dpad: rgb(0xB3_94_FF),
            joystick: rgb(0xB3_94_FF),
            system: rgb(0xFF_88_9C),
            bg_binding: rgb(0x2A_2A_1E),
            ink_1: rgb(0xE8_E6_E0),
            ink_2: rgb(0x9A_A0_A8),
            ink_3: rgb(0x5E_64_6C),
            line: rgb(0x2A_2F_36),
        }
    }
}

/// Map an `Output` to its category color for binding rows + map badges.
#[must_use]
pub const fn output_color(palette: &Palette, output: &yoke_config::catalog::Output) -> Color32 {
    use yoke_config::catalog::Output;
    match output {
        Output::Keyboard(_) => palette.keyboard,
        Output::Mouse(_) => palette.mouse,
        Output::Gamepad(_) => palette.gamepad,
        Output::Dpad(_) => palette.dpad,
        Output::Joystick(_) => palette.joystick,
        Output::System(_) => palette.system,
        Output::Touch | Output::Unknown(_) => palette.ink_2,
    }
}

/// Map a category label (as produced by `output_category`) to its palette hue.
///
/// The labels match the design's `OUTPUT_CATEGORIES` ids one-to-one; unknown
/// labels fall back to `ink_2`, matching `output_color`'s Touch/Unknown case.
#[must_use]
pub fn category_color(palette: &Palette, category: &str) -> Color32 {
    match category {
        "Keyboard" => palette.keyboard,
        "Mouse" => palette.mouse,
        "Gamepad" => palette.gamepad,
        "Dpad" => palette.dpad,
        "Joystick" => palette.joystick,
        "System" => palette.system,
        "Touch" => palette.accent,
        _ => palette.ink_2,
    }
}

// Console surface tokens (design `[data-theme="console"]`).
const BG_0: Color32 = rgb(0x0A_0B_0D); // app frame
const BG_1: Color32 = rgb(0x14_16_1A); // canvas / panels
pub(crate) const BG_2: Color32 = rgb(0x1B_1E_23); // surface / ghost-button fill
pub(crate) const BG_3: Color32 = rgb(0x20_24_2A); // subtle surface / hover
pub(crate) const BG_4: Color32 = rgb(0x2A_2F_36); // depressed / active
pub(crate) const INK_1: Color32 = rgb(0xE8_E6_E0);
pub(crate) const INK_2: Color32 = rgb(0x9A_A0_A8);
const INK_3: Color32 = rgb(0x5E_64_6C);
pub(crate) const LINE: Color32 = rgb(0x2A_2F_36);
const LINE_STRONG: Color32 = rgb(0x3A_40_48);
const ACCENT: Color32 = rgb(0x84_C4_85);
const ACCENT_2: Color32 = rgb(0x21_33_21);
const BG_BINDING: Color32 = rgb(0x2A_2A_1E);

// Corner-radius scale; widgets read u8. One source of truth for the values
// used in more than one place.
pub const R_SM: u8 = 6; // --r-sm
pub const R_MD: u8 = 10; // --r-md
pub const R_FULL: u8 = 99; // fully-rounded pill

/// Console dark `egui::Visuals` from the design `--bg-*` / `--ink-*` tokens.
///
/// Widgets are styled as the design's ghost buttons: `--bg-2` fill, a `--line`
/// border, `--r-sm` corners, lifting to `--bg-3`/`--bg-4` on hover/press.
#[must_use]
pub fn console_visuals() -> egui::Visuals {
    use egui::{CornerRadius, Stroke};
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG_1;
    v.window_fill = BG_2;
    v.extreme_bg_color = BG_0;
    v.faint_bg_color = BG_3;
    v.override_text_color = Some(INK_1);
    v.hyperlink_color = ACCENT;
    v.selection.bg_fill = ACCENT_2;
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.window_stroke = Stroke::new(1.0, LINE);
    v.window_corner_radius = CornerRadius::same(R_MD);
    v.menu_corner_radius = CornerRadius::same(R_SM);

    let radius = CornerRadius::same(R_SM);
    let w = &mut v.widgets;
    // Separators, frame outlines, non-clickable text.
    w.noninteractive.bg_fill = BG_1;
    w.noninteractive.weak_bg_fill = BG_1;
    w.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    w.noninteractive.fg_stroke = Stroke::new(1.0, INK_2);
    w.noninteractive.corner_radius = radius;
    // Resting buttons.
    w.inactive.bg_fill = BG_2;
    w.inactive.weak_bg_fill = BG_2;
    w.inactive.bg_stroke = Stroke::new(1.0, LINE);
    w.inactive.fg_stroke = Stroke::new(1.0, INK_1);
    w.inactive.corner_radius = radius;
    // Hover.
    w.hovered.bg_fill = BG_3;
    w.hovered.weak_bg_fill = BG_3;
    w.hovered.bg_stroke = Stroke::new(1.0, LINE_STRONG);
    w.hovered.fg_stroke = Stroke::new(1.0, INK_1);
    w.hovered.corner_radius = radius;
    // Pressed.
    w.active.bg_fill = BG_4;
    w.active.weak_bg_fill = BG_4;
    w.active.bg_stroke = Stroke::new(1.0, ACCENT);
    w.active.fg_stroke = Stroke::new(1.0, INK_1);
    w.active.corner_radius = radius;
    // Open combo/menu.
    w.open.bg_fill = BG_3;
    w.open.weak_bg_fill = BG_3;
    w.open.bg_stroke = Stroke::new(1.0, LINE_STRONG);
    w.open.fg_stroke = Stroke::new(1.0, INK_1);
    w.open.corner_radius = radius;
    v
}

/// Build the full Console `Style`.
///
/// Layers the design typography scale (Instrument serif display headings,
/// `JetBrains` Mono eyebrows/labels, Manrope body) and roomier spacing on
/// `console_visuals`.
#[must_use]
pub fn console_style() -> egui::Style {
    use egui::{FontFamily, FontId, TextStyle};

    let mut style = egui::Style::default();
    let serif = FontFamily::Name("Instrument".into());
    style.text_styles = [
        (TextStyle::Heading, FontId::new(30.0, serif)),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(13.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(12.5, FontFamily::Monospace),
        ),
        // Eyebrows/captions render mono, matching the design's small labels.
        (TextStyle::Small, FontId::new(11.0, FontFamily::Monospace)),
    ]
    .into();

    let s = &mut style.spacing;
    s.item_spacing = egui::vec2(8.0, 8.0);
    s.button_padding = egui::vec2(12.0, 7.0);
    s.interact_size.y = 30.0;
    s.indent = 18.0;
    s.menu_margin = egui::Margin::same(6);

    style.visuals = console_visuals();
    style
}

/// Install the Console theme on a context. Call once at startup.
pub fn apply(ctx: &egui::Context) {
    ctx.set_global_style(console_style());
}

/// Surface card for the editor's device/bindings panes (design
/// `.dev-pane`/`.bind-pane`): `--bg-2` fill, `--line` border, `--r-md` corners.
pub fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(BG_2)
        .stroke(egui::Stroke::new(1.0, LINE))
        .corner_radius(egui::CornerRadius::same(R_MD))
        .inner_margin(egui::Margin::symmetric(16, 14))
}

/// Small rounded pill (design `.mod-pill`): `--bg-2` fill, `--line` border.
pub fn pill_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(BG_2)
        .stroke(egui::Stroke::new(1.0, LINE))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(7, 2))
}

/// Bordered binding-row container (design `.brow`): `--bg-1`, `--line` border.
pub fn row_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(BG_1)
        .stroke(egui::Stroke::new(1.0, LINE))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(10, 7))
}

/// Show a frame, then promote its whole rect to a pointer-cursor click target.
///
/// `id_salt` disambiguates the interaction id from the frame's own id so
/// repeated chips in a loop don't collide.
pub fn clickable_frame<R>(
    ui: &mut egui::Ui,
    frame: egui::Frame,
    id_salt: impl std::hash::Hash,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::Response {
    let resp = frame.show(ui, add_contents).response;
    ui.interact(resp.rect, resp.id.with(id_salt), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Filled, bordered output button (design `.brow-out.set`).
///
/// `--bg-binding` fill and a solid `--line` border with `--r-sm` corners,
/// containing a category-colored output glyph and an `--ink-1` label. Returns
/// the button's response so the caller can wire it to the existing edit-output
/// picker. Visual container only — it carries no mutation itself.
pub fn output_button(
    ui: &mut egui::Ui,
    glyph: &str,
    label: &str,
    color: egui::Color32,
) -> egui::Response {
    let frame = egui::Frame::new()
        .fill(BG_BINDING)
        .stroke(egui::Stroke::new(1.0, LINE))
        .corner_radius(egui::CornerRadius::same(R_SM))
        .inner_margin(egui::Margin::symmetric(8, 5));
    clickable_frame(ui, frame, label, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 7.0;
            ui.add(egui::Label::new(
                egui::RichText::new(glyph)
                    .monospace()
                    .strong()
                    .size(12.0)
                    .color(color),
            ));
            ui.add(egui::Label::new(egui::RichText::new(label).color(INK_1).size(12.5)).truncate());
        });
    })
}

/// Unset output button (design `.brow-out.empty`): a faint solid border (egui
/// has no dashed strokes) around `--ink-3` "+ Bind output" text.
pub fn empty_output_button(ui: &mut egui::Ui, id_salt: impl std::hash::Hash) -> egui::Response {
    let frame = egui::Frame::new()
        .stroke(egui::Stroke::new(1.0, LINE))
        .corner_radius(egui::CornerRadius::same(R_SM))
        .inner_margin(egui::Margin::symmetric(8, 5));
    clickable_frame(ui, frame, id_salt, |ui| {
        ui.label(egui::RichText::new("+ Bind output").size(12.5).color(INK_3));
    })
}

/// Modifier pill frame + text color.
///
/// A plain `normal` modifier renders borderless and dimmed, brightening while
/// the pointer is over its row (design `.mod-pill.quiet`); any other modifier
/// keeps the standard pill (design `.mod-pill`).
pub fn mod_pill_style(
    palette: &Palette,
    modifier: &str,
    row_hovered: bool,
) -> (egui::Frame, Color32) {
    if modifier != "normal" {
        return (pill_frame(), palette.ink_2);
    }
    let quiet = egui::Frame::new()
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(7, 2));
    if row_hovered {
        (
            quiet.stroke(egui::Stroke::new(1.0, LINE)),
            palette.ink_2.gamma_multiply(0.8),
        )
    } else {
        (quiet, palette.ink_2.gamma_multiply(0.4))
    }
}

/// A small keycap hint (design `kbd`): `--bg-3` fill, `--line` border, mono
/// `--ink-3` glyph. Display-only label for keyboard affordances like "esc".
pub fn kbd_hint(ui: &mut egui::Ui, label: &str, palette: &Palette) {
    egui::Frame::new()
        .fill(BG_3)
        .stroke(egui::Stroke::new(1.0, LINE))
        .corner_radius(egui::CornerRadius::same(4))
        .inner_margin(egui::Margin::symmetric(5, 1))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(label)
                    .monospace()
                    .size(10.0)
                    .color(palette.ink_3),
            );
        });
}

/// Bottom status bar (design `.status`): `--bg-3` fill, `--line` stroke.
/// Callers add `inner_margin` to control padding.
pub fn status_bar_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(BG_3)
        .stroke(egui::Stroke::new(1.0, LINE))
}

/// Segmented container for the sub-profile tab strip (design `.sub-tabs`):
/// `--bg-3` fill, `--line` border, `--r-md` corners.
pub fn strip_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(BG_3)
        .stroke(egui::Stroke::new(1.0, LINE))
        .corner_radius(egui::CornerRadius::same(R_MD))
        .inner_margin(egui::Margin::same(4))
}

/// Mono uppercase eyebrow label at 11px in `INK_2`.
#[must_use]
pub fn eyebrow(text: &str) -> egui::RichText {
    egui::RichText::new(text.to_uppercase())
        .monospace()
        .size(11.0)
        .color(INK_2)
}

/// Filled primary action button (design `.btn-primary`): `INK_1` fill and
/// border, `BG_1` label.
pub fn primary_button(text: &str) -> egui::Button<'_> {
    egui::Button::new(egui::RichText::new(text).color(BG_1).strong())
        .fill(INK_1)
        .stroke(egui::Stroke::new(1.0, INK_1))
}

/// One sub-profile chip container (design `.sub-tab` / `.sub-tab.on`):
/// transparent when resting, a `--bg-2` fill with a `--line` border when
/// selected. 6px corners, 6px×10px padding.
pub fn sub_tab_frame(selected: bool) -> egui::Frame {
    let (fill, stroke) = if selected {
        (BG_2, egui::Stroke::new(1.0, LINE))
    } else {
        (Color32::TRANSPARENT, egui::Stroke::NONE)
    };
    egui::Frame::new()
        .fill(fill)
        .stroke(stroke)
        .corner_radius(egui::CornerRadius::same(R_SM))
        .inner_margin(egui::Margin::symmetric(10, 6))
}

/// Neutral profile-kind tag (design `.kind-tag`): `--bg-3` fill, mono
/// `--ink-2` text, 4px corners.
pub fn kind_badge(ui: &mut egui::Ui, label: &str) {
    egui::Frame::new()
        .fill(BG_3)
        .corner_radius(egui::CornerRadius::same(4))
        .inner_margin(egui::Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(label)
                    .monospace()
                    .size(10.5)
                    .color(INK_2),
            );
        });
}

/// Segmented selector (design `.seg`): `--bg-3` strip, `--line` border, 3px padding.
///
/// Chips are painted directly so the selected state uses the design's
/// `--bg-2` + `--line` look instead of egui's global selection tint.
/// Returns the newly-selected index when the selection changes.
pub fn segmented(ui: &mut egui::Ui, labels: &[&str], selected: usize) -> Option<usize> {
    let mut changed = None;
    egui::Frame::new()
        .fill(BG_3)
        .stroke(egui::Stroke::new(1.0, LINE))
        .corner_radius(egui::CornerRadius::same(R_SM))
        .inner_margin(egui::Margin::same(3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for (i, l) in labels.iter().enumerate() {
                    if seg_chip(ui, l, i == selected).clicked() && i != selected {
                        changed = Some(i);
                    }
                }
            });
        });
    changed
}

/// One segmented-control chip (design `.seg button` / `.seg button.on`).
fn seg_chip(ui: &mut egui::Ui, label: &str, on: bool) -> egui::Response {
    let color = if on { INK_1 } else { INK_2 };
    let galley =
        ui.painter()
            .layout_no_wrap(label.to_owned(), egui::FontId::proportional(12.5), color);
    let size = galley.size() + egui::vec2(24.0, 8.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if on {
            painter.rect(
                rect,
                egui::CornerRadius::same(4),
                BG_2,
                egui::Stroke::new(1.0, LINE),
                egui::StrokeKind::Inside,
            );
        } else if response.hovered() {
            // BG_3 would vanish against the BG_3 strip; one step up instead.
            painter.rect_filled(rect, egui::CornerRadius::same(4), BG_4);
        }
        painter.galley(rect.center() - galley.size() / 2.0, galley, color);
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Left-rail navigation item (design `.side-item` / `.side-item.active`):
/// active = `--bg-2` fill + `--line` border + `--ink-1`; inactive =
/// transparent + `--ink-2`, hover `--bg-4`. Spans the rail width.
pub fn nav_item(ui: &mut egui::Ui, label: &str, active: bool) -> egui::Response {
    let color = if active { INK_1 } else { INK_2 };
    let galley =
        ui.painter()
            .layout_no_wrap(label.to_owned(), egui::FontId::proportional(13.0), color);
    let size = egui::vec2(ui.available_width(), galley.size().y + 16.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if active {
            painter.rect(
                rect,
                egui::CornerRadius::same(R_SM),
                BG_2,
                egui::Stroke::new(1.0, LINE),
                egui::StrokeKind::Inside,
            );
        } else if response.hovered() {
            painter.rect_filled(rect, egui::CornerRadius::same(R_SM), BG_4);
        }
        let pos = egui::pos2(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0);
        painter.galley(pos, galley, color);
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Search input in a design pill (design `.search-pill`).
///
/// Fixed 280x32, `--bg-2` fill, `--line` border, painted magnifier glyph.
/// The `TextEdit` is frameless so egui's accent focus ring never shows.
pub fn search_pill(ui: &mut egui::Ui, text: &mut String, hint: &str) {
    egui::Frame::new()
        .fill(BG_2)
        .stroke(egui::Stroke::new(1.0, LINE))
        .corner_radius(egui::CornerRadius::same(R_SM))
        .inner_margin(egui::Margin::symmetric(10, 0))
        .show(ui, |ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(260.0, 32.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    magnifier_icon(ui);
                    // A custom frame suppresses egui's own fill and accent
                    // focus ring (design: the pill is the only chrome).
                    ui.add(
                        egui::TextEdit::singleline(text)
                            .frame(egui::Frame::new())
                            .hint_text(hint)
                            .desired_width(ui.available_width()),
                    );
                },
            );
        });
}

/// Stroked magnifier glyph matching the design's inline SVG.
fn magnifier_icon(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
    let stroke = egui::Stroke::new(1.2, INK_2);
    let dir = egui::vec2(
        std::f32::consts::FRAC_1_SQRT_2,
        std::f32::consts::FRAC_1_SQRT_2,
    );
    // Nudge the lens up-left so the handle stays inside the icon box.
    let center = rect.center() - egui::vec2(1.5, 1.5);
    let painter = ui.painter();
    painter.circle_stroke(center, 4.5, stroke);
    painter.line_segment([center + 4.5 * dir, center + 8.0 * dir], stroke);
}

/// Embed OFL-licensed fonts and register them with egui. Call once at startup,
/// before `apply`, so visuals reference the correct families.
pub fn install_fonts(ctx: &egui::Context) {
    use egui::{FontData, FontDefinitions, FontFamily};
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        "Manrope".to_owned(),
        std::sync::Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/manrope/Manrope-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        "JetBrainsMono".to_owned(),
        std::sync::Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/jetbrains-mono/JetBrainsMono-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        "InstrumentSerif".to_owned(),
        std::sync::Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/instrument-serif/InstrumentSerif-Regular.ttf"
        ))),
    );

    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "Manrope".to_owned());
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, "JetBrainsMono".to_owned());
    // Named family for display headings; not a standard egui family.
    fonts.families.insert(
        FontFamily::Name("Instrument".into()),
        vec!["InstrumentSerif".to_owned()],
    );

    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_palette_accent_is_the_expected_green() {
        let p = Palette::console();
        assert_eq!(p.accent, Color32::from_rgb(0x84, 0xC4, 0x85));
        assert_eq!(p.accent_2, Color32::from_rgb(0x21, 0x33, 0x21));
    }
}
