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

/// Console dark `egui::Visuals` built from the design `--bg-*` / `--ink-*` tokens.
#[must_use]
pub fn console_visuals() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.panel_fill = rgb(0x14_16_1A); // --bg-1
    v.window_fill = rgb(0x1B_1E_23); // --bg-2
    v.extreme_bg_color = rgb(0x0A_0B_0D); // --bg-0
    v.faint_bg_color = rgb(0x20_24_2A); // --bg-3
    v.override_text_color = Some(rgb(0xE8_E6_E0)); // --ink-1
    v.hyperlink_color = rgb(0x6E_D2_74); // --accent
    v.selection.bg_fill = rgb(0x1E_3B_1F); // --accent-2
    v.selection.stroke = egui::Stroke::new(1.0, rgb(0x6E_D2_74));
    let line = rgb(0x2A_2F_36); // --line
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, line);
    v.widgets.inactive.bg_fill = rgb(0x20_24_2A); // --bg-3
    v.widgets.hovered.bg_fill = rgb(0x2A_2F_36); // --bg-4
    v
}

/// Install the Console theme on a context. Call once at startup.
pub fn apply(ctx: &egui::Context) {
    ctx.set_visuals(console_visuals());
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
