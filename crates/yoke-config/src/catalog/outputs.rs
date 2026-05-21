use serde::{Deserialize, Serialize};

use super::DPadDir;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Output {
    Keyboard(KbKey),
    Mouse(MouseAction),
    Gamepad(GamepadButton),
    Dpad(DPadDir),
    Joystick(JoyOutput),
    System(SystemAction),
    Touch,
    Unknown(String),
}

macro_rules! csv_enum {
    ( $name:ident { $( $variant:ident => $csv:literal ),+ $(,)? } ) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum $name { $( $variant ),+ }
        impl $name {
            pub const ALL: &'static [Self] = &[ $( Self::$variant ),+ ];
            pub const fn as_csv(self) -> &'static str {
                match self { $( Self::$variant => $csv ),+ }
            }
            pub fn from_csv(s: &str) -> Option<Self> {
                match s {
                    $( $csv => Some(Self::$variant), )+
                    _ => None,
                }
            }
        }
    };
}

csv_enum! { KbKey {
    A => "kb_a", B => "kb_b", C => "kb_c", D => "kb_d", E => "kb_e",
    F => "kb_f", G => "kb_g", H => "kb_h", I => "kb_i", J => "kb_j",
    K => "kb_k", L => "kb_l", M => "kb_m", N => "kb_n", O => "kb_o",
    P => "kb_p", Q => "kb_q", R => "kb_r", S => "kb_s", T => "kb_t",
    U => "kb_u", V => "kb_v", W => "kb_w", X => "kb_x", Y => "kb_y",
    Z => "kb_z",
    Digit0 => "kb_0", Digit1 => "kb_1", Digit2 => "kb_2",
    Digit3 => "kb_3", Digit4 => "kb_4", Digit5 => "kb_5",
    Digit6 => "kb_6", Digit7 => "kb_7", Digit8 => "kb_8", Digit9 => "kb_9",
    F1 => "kb_f1",  F2 => "kb_f2",  F3 => "kb_f3",  F4 => "kb_f4",
    F5 => "kb_f5",  F6 => "kb_f6",  F7 => "kb_f7",  F8 => "kb_f8",
    F9 => "kb_f9",  F10 => "kb_f10", F11 => "kb_f11", F12 => "kb_f12",
    Space     => "kb_space",
    Enter     => "kb_enter",
    Escape    => "kb_escape",
    Tab       => "kb_tab",
    Backspace => "kb_backspace",
    Delete    => "kb_delete",
    LeftShift   => "kb_left_shift",   RightShift   => "kb_right_shift",
    LeftControl => "kb_left_control", RightControl => "kb_right_control",
    LeftAlt     => "kb_left_alt",     RightAlt     => "kb_right_alt",
    LeftGui     => "kb_left_gui",     RightGui     => "kb_right_gui",
    UpArrow     => "kb_up_arrow",     DownArrow    => "kb_down_arrow",
    LeftArrow   => "kb_left_arrow",   RightArrow   => "kb_right_arrow",
    Slash       => "kb_slash",        KeypadPlus   => "kb_keypad_plus",
}}

csv_enum! { MouseAction {
    Left         => "mouse_left",
    Right        => "mouse_right",
    Up           => "mouse_up",
    Down         => "mouse_down",
    WheelUp      => "mouse_wheel_up",
    WheelDown    => "mouse_wheel_down",
    PanLeft      => "mouse_pan_left",
    PanRight     => "mouse_pan_right",
    LeftButton   => "mouse_left_button",
    RightButton  => "mouse_right_button",
    MiddleButton => "mouse_middle_button",
}}

csv_enum! { GamepadButton {
    Cross    => "x",        Circle   => "circle",
    Square   => "square",   Triangle => "triangle",
    L1 => "left_1",  L2 => "left_2",  L3 => "left_3",
    R1 => "right_1", R2 => "right_2", R3 => "right_3",
    Select => "select", Start => "start", PsHome => "ps3",
    A => "A", B => "B", X => "X", Y => "Y",
    LeftBumper  => "left_bumper",  LeftTrigger  => "left_trigger",  LeftStick  => "left_stick",
    RightBumper => "right_bumper", RightTrigger => "right_trigger", RightStick => "right_stick",
    Back    => "back",
    Guide   => "guide",
    Capture => "capture",
}}

csv_enum! { JoyOutput {
    LeftJoyLeft   => "left_joy_left",
    LeftJoyRight  => "left_joy_right",
    LeftJoyUp     => "left_joy_up",
    LeftJoyDown   => "left_joy_down",
    RightJoyLeft  => "right_joy_left",
    RightJoyRight => "right_joy_right",
    RightJoyUp    => "right_joy_up",
    RightJoyDown  => "right_joy_down",
}}

csv_enum! { SystemAction {
    IncrementMode => "increment_mode",
    DecrementMode => "decrement_mode",
}}

impl Output {
    /// Parse a non-empty CSV output identifier. Empty strings are caller-filtered
    /// (they signal a blank-output terminator row at the parser level).
    /// Unknown identifiers become `Output::Unknown(s)` so they survive round-trip.
    pub fn from_csv(s: &str) -> Self {
        debug_assert!(!s.is_empty(), "Output::from_csv called with empty string");
        if s == "touch" {
            return Self::Touch;
        }
        if let Some(v) = KbKey::from_csv(s) {
            return Self::Keyboard(v);
        }
        if let Some(v) = MouseAction::from_csv(s) {
            return Self::Mouse(v);
        }
        if let Some(v) = GamepadButton::from_csv(s) {
            return Self::Gamepad(v);
        }
        if let Some(rest) = s.strip_prefix("dpad_")
            && let Some(d) = DPadDir::from_csv(rest)
        {
            return Self::Dpad(d);
        }
        if let Some(v) = JoyOutput::from_csv(s) {
            return Self::Joystick(v);
        }
        if let Some(v) = SystemAction::from_csv(s) {
            return Self::System(v);
        }
        Self::Unknown(s.to_owned())
    }

    pub fn to_csv(&self) -> String {
        match self {
            Self::Keyboard(k) => k.as_csv().to_owned(),
            Self::Mouse(m) => m.as_csv().to_owned(),
            Self::Gamepad(g) => g.as_csv().to_owned(),
            Self::Dpad(d) => format!("dpad_{}", d.as_csv()),
            Self::Joystick(j) => j.as_csv().to_owned(),
            Self::System(s) => s.as_csv().to_owned(),
            Self::Touch => "touch".into(),
            Self::Unknown(s) => s.clone(),
        }
    }

    /// Enumerate every canonical `Output` value (everything except `Unknown`).
    pub fn iter_known() -> impl Iterator<Item = Self> {
        let kb = KbKey::ALL.iter().copied().map(Self::Keyboard);
        let mouse = MouseAction::ALL.iter().copied().map(Self::Mouse);
        let gamepad = GamepadButton::ALL.iter().copied().map(Self::Gamepad);
        let dpad = DPadDir::ALL.iter().copied().map(Self::Dpad);
        let joy = JoyOutput::ALL.iter().copied().map(Self::Joystick);
        let sys = SystemAction::ALL.iter().copied().map(Self::System);
        kb.chain(mouse)
            .chain(gamepad)
            .chain(dpad)
            .chain(joy)
            .chain(sys)
            .chain(std::iter::once(Self::Touch))
    }

    /// CSV identifiers for every known `Output` variant (`iter_known().map(to_csv)`).
    pub fn all_csv_names() -> impl Iterator<Item = String> {
        Self::iter_known().map(|o| o.to_csv())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Source: https://quadstick.s3.amazonaws.com/documents/user_manual/um/dropdown_list_for_outputs.htm
    // and its linked PlayStation/Xbox subpages. Fred's current template adds `touch`.
    const ALL_OUTPUT_IDS: &[&str] = &[
        // Keyboard letters
        "kb_a",
        "kb_b",
        "kb_c",
        "kb_d",
        "kb_e",
        "kb_f",
        "kb_g",
        "kb_h",
        "kb_i",
        "kb_j",
        "kb_k",
        "kb_l",
        "kb_m",
        "kb_n",
        "kb_o",
        "kb_p",
        "kb_q",
        "kb_r",
        "kb_s",
        "kb_t",
        "kb_u",
        "kb_v",
        "kb_w",
        "kb_x",
        "kb_y",
        "kb_z",
        // Keyboard digits
        "kb_0",
        "kb_1",
        "kb_2",
        "kb_3",
        "kb_4",
        "kb_5",
        "kb_6",
        "kb_7",
        "kb_8",
        "kb_9",
        // Keyboard function row
        "kb_f1",
        "kb_f2",
        "kb_f3",
        "kb_f4",
        "kb_f5",
        "kb_f6",
        "kb_f7",
        "kb_f8",
        "kb_f9",
        "kb_f10",
        "kb_f11",
        "kb_f12",
        // Keyboard specials
        "kb_space",
        "kb_enter",
        "kb_escape",
        "kb_tab",
        "kb_backspace",
        "kb_delete",
        "kb_left_shift",
        "kb_right_shift",
        "kb_left_control",
        "kb_right_control",
        "kb_left_alt",
        "kb_right_alt",
        "kb_left_gui",
        "kb_right_gui",
        "kb_up_arrow",
        "kb_down_arrow",
        "kb_left_arrow",
        "kb_right_arrow",
        "kb_slash",
        "kb_keypad_plus",
        // Mouse
        "mouse_left",
        "mouse_right",
        "mouse_up",
        "mouse_down",
        "mouse_wheel_up",
        "mouse_wheel_down",
        "mouse_pan_left",
        "mouse_pan_right",
        "mouse_left_button",
        "mouse_right_button",
        "mouse_middle_button",
        // Gamepad PlayStation
        "x",
        "circle",
        "square",
        "triangle",
        "left_1",
        "left_2",
        "left_3",
        "right_1",
        "right_2",
        "right_3",
        "select",
        "start",
        "ps3",
        "touch",
        // Gamepad Xbox / Switch
        "A",
        "B",
        "X",
        "Y",
        "left_bumper",
        "left_trigger",
        "left_stick",
        "right_bumper",
        "right_trigger",
        "right_stick",
        "back",
        "guide",
        "capture",
        // D-pad outputs
        "dpad_N",
        "dpad_NE",
        "dpad_E",
        "dpad_SE",
        "dpad_S",
        "dpad_SW",
        "dpad_W",
        "dpad_NW",
        // Joystick outputs
        "left_joy_left",
        "left_joy_right",
        "left_joy_up",
        "left_joy_down",
        "right_joy_left",
        "right_joy_right",
        "right_joy_up",
        "right_joy_down",
        // System
        "increment_mode",
        "decrement_mode",
    ];

    #[test]
    fn every_documented_id_round_trips() {
        for id in ALL_OUTPUT_IDS {
            let parsed = Output::from_csv(id);
            assert!(
                !matches!(parsed, Output::Unknown(_)),
                "{id} parsed as Unknown"
            );
            assert_eq!(parsed.to_csv(), *id, "{id} did not round-trip");
        }
    }

    #[test]
    fn unknown_output_round_trips_verbatim() {
        let parsed = Output::from_csv("mystery_output");
        assert_eq!(parsed, Output::Unknown("mystery_output".into()));
        assert_eq!(parsed.to_csv(), "mystery_output");
    }

    #[test]
    fn touch_is_a_dedicated_variant() {
        assert_eq!(Output::from_csv("touch"), Output::Touch);
    }

    #[test]
    fn iter_known_covers_every_documented_id() {
        let enumerated: std::collections::HashSet<String> = Output::all_csv_names().collect();
        for id in ALL_OUTPUT_IDS {
            assert!(
                enumerated.contains(*id),
                "iter_known missing documented id: {id}"
            );
        }
    }

    #[test]
    fn iter_known_round_trips_every_variant() {
        for variant in Output::iter_known() {
            let csv = variant.to_csv();
            let parsed = Output::from_csv(&csv);
            assert_eq!(parsed, variant, "round-trip failed for {csv}");
        }
    }
}
