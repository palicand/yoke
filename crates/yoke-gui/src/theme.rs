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
            accent: rgb(0x6E_D2_74),
            accent_2: rgb(0x1E_3B_1F),
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

// Console surface tokens (design `[data-theme="console"]`).
const BG_0: Color32 = rgb(0x0A_0B_0D); // app frame
const BG_1: Color32 = rgb(0x14_16_1A); // canvas / panels
const BG_2: Color32 = rgb(0x1B_1E_23); // surface / ghost-button fill
const BG_3: Color32 = rgb(0x20_24_2A); // subtle surface / hover
const BG_4: Color32 = rgb(0x2A_2F_36); // depressed / active
const INK_1: Color32 = rgb(0xE8_E6_E0);
const INK_2: Color32 = rgb(0x9A_A0_A8);
const LINE: Color32 = rgb(0x2A_2F_36);
const LINE_STRONG: Color32 = rgb(0x3A_40_48);
const ACCENT: Color32 = rgb(0x6E_D2_74);
const ACCENT_2: Color32 = rgb(0x1E_3B_1F);

// Corner radius `--r-sm`; widgets read u8.
const R_SM: u8 = 6;

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
    v.window_corner_radius = CornerRadius::same(10); // --r-md
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
        .corner_radius(egui::CornerRadius::same(10))
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

/// Segmented container for the sub-profile tab strip (design `.sub-tabs`):
/// `--bg-3` fill, `--line` border, `--r-md` corners.
pub fn strip_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(BG_3)
        .stroke(egui::Stroke::new(1.0, LINE))
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin::same(4))
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
        assert_eq!(p.accent, Color32::from_rgb(0x6E, 0xD2, 0x74));
    }
}
