use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MpPosition {
    Left,
    LeftCenter,
    Center,
    RightCenter,
    Right,
    LeftRight,
    Triple,
}

impl MpPosition {
    pub const ALL: &'static [Self] = &[
        Self::Left,
        Self::LeftCenter,
        Self::Center,
        Self::RightCenter,
        Self::Right,
        Self::LeftRight,
        Self::Triple,
    ];

    pub const fn as_csv(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::LeftCenter => "left_center",
            Self::Center => "center",
            Self::RightCenter => "right_center",
            Self::Right => "right",
            Self::LeftRight => "left_right",
            Self::Triple => "triple",
        }
    }

    pub fn from_csv(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|p| p.as_csv() == s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SipPuff {
    Sip,
    Puff,
}

impl SipPuff {
    pub const ALL: &'static [Self] = &[Self::Sip, Self::Puff];

    pub const fn as_csv(self) -> &'static str {
        match self {
            Self::Sip => "sip",
            Self::Puff => "puff",
        }
    }
    pub fn from_csv(s: &str) -> Option<Self> {
        match s {
            "sip" => Some(Self::Sip),
            "puff" => Some(Self::Puff),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum JoyAxis {
    Left,
    Right,
    Up,
    Down,
}

impl JoyAxis {
    pub const ALL: &'static [Self] = &[Self::Left, Self::Right, Self::Up, Self::Down];
    pub const fn as_csv(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Up => "up",
            Self::Down => "down",
        }
    }
    pub fn from_csv(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|d| d.as_csv() == s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DPadDir {
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
    NW,
}

impl DPadDir {
    pub const ALL: &'static [Self] = &[
        Self::N,
        Self::NE,
        Self::E,
        Self::SE,
        Self::S,
        Self::SW,
        Self::W,
        Self::NW,
    ];
    pub const fn as_csv(self) -> &'static str {
        match self {
            Self::N => "N",
            Self::NE => "NE",
            Self::E => "E",
            Self::SE => "SE",
            Self::S => "S",
            Self::SW => "SW",
            Self::W => "W",
            Self::NW => "NW",
        }
    }
    pub fn from_csv(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|d| d.as_csv() == s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UsbHost {
    One,
    Two,
}

impl UsbHost {
    pub const ALL: &'static [Self] = &[Self::One, Self::Two];
    pub const fn as_csv_index(self) -> &'static str {
        match self {
            Self::One => "1",
            Self::Two => "2",
        }
    }
    pub fn from_csv_index(s: &str) -> Option<Self> {
        match s {
            "1" => Some(Self::One),
            "2" => Some(Self::Two),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SideKind {
    Hard,
    Soft,
    Long,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Input {
    Mouthpiece {
        pos: MpPosition,
        dir: SipPuff,
        soft: bool,
    },
    Side {
        dir: SipPuff,
        kind: SideKind,
    },
    Lip {
        soft: bool,
    },
    JoystickAxis(JoyAxis),
    JoystickDpad {
        dir: DPadDir,
        inner: bool,
    },
    JoystickAnyDirection,
    Center,
    Constant,
    UsbHostAxis {
        host: UsbHost,
        axis: JoyAxis,
    },
    UsbHostDpad {
        host: UsbHost,
        dir: DPadDir,
        inner: bool,
    },
    UsbHostButton {
        host: UsbHost,
        button: u8,
    },
    DigitalIn(u8),
    Unknown(String),
}

impl Input {
    /// Parse a non-empty CSV input identifier. Empty strings are caller-filtered
    /// (they mean "no input bound" in the source row, not a missing variant).
    /// Unknown identifiers become `Input::Unknown(s)` so they survive round-trip.
    pub fn from_csv(s: &str) -> Self {
        debug_assert!(!s.is_empty(), "Input::from_csv called with empty string");
        parse_input(s).unwrap_or_else(|| Self::Unknown(s.to_owned()))
    }

    pub fn to_csv(&self) -> String {
        match self {
            Self::Mouthpiece { pos, dir, soft } => {
                let suffix = if *soft { "_soft" } else { "" };
                format!("mp_{}_{}{}", pos.as_csv(), dir.as_csv(), suffix)
            }
            Self::Side { dir, kind } => {
                let suffix = match kind {
                    SideKind::Hard => "",
                    SideKind::Soft => "_soft",
                    SideKind::Long => "_long",
                };
                format!("right_{}{}", dir.as_csv(), suffix)
            }
            Self::Lip { soft } => {
                if *soft {
                    "lip_soft".into()
                } else {
                    "lip".into()
                }
            }
            Self::JoystickAxis(ax) => ax.as_csv().to_owned(),
            Self::JoystickDpad { dir, inner } => {
                if *inner {
                    format!("{}_inner", dir.as_csv())
                } else {
                    dir.as_csv().to_owned()
                }
            }
            Self::JoystickAnyDirection => "any_direction".into(),
            Self::Center => "center".into(),
            Self::Constant => "constant".into(),
            Self::UsbHostAxis { host, axis } => {
                format!("usb_{}_{}", host.as_csv_index(), axis.as_csv())
            }
            Self::UsbHostDpad { host, dir, inner } => {
                if *inner {
                    format!("usb_{}_{}_inner", host.as_csv_index(), dir.as_csv())
                } else {
                    format!("usb_{}_{}", host.as_csv_index(), dir.as_csv())
                }
            }
            Self::UsbHostButton { host, button } => {
                format!("usb_{}_button_{}", host.as_csv_index(), button)
            }
            Self::DigitalIn(n) => format!("digital_in_{n}"),
            Self::Unknown(s) => s.clone(),
        }
    }
}

fn parse_input(s: &str) -> Option<Input> {
    if let Some(rest) = s.strip_prefix("mp_") {
        return parse_mp(rest);
    }
    if let Some(rest) = s.strip_prefix("right_") {
        return parse_side(rest);
    }
    if s == "lip" {
        return Some(Input::Lip { soft: false });
    }
    if s == "lip_soft" {
        return Some(Input::Lip { soft: true });
    }
    if let Some(ax) = JoyAxis::from_csv(s) {
        return Some(Input::JoystickAxis(ax));
    }
    if s == "any_direction" {
        return Some(Input::JoystickAnyDirection);
    }
    if s == "center" {
        return Some(Input::Center);
    }
    if s == "constant" {
        return Some(Input::Constant);
    }
    if let Some((dir, inner)) = parse_dpad(s) {
        return Some(Input::JoystickDpad { dir, inner });
    }
    if let Some(rest) = s.strip_prefix("usb_1_") {
        return parse_usb_host(UsbHost::One, rest);
    }
    if let Some(rest) = s.strip_prefix("usb_2_") {
        return parse_usb_host(UsbHost::Two, rest);
    }
    if let Some(n_str) = s.strip_prefix("digital_in_")
        && let Ok(n) = n_str.parse::<u8>()
        && (1..=8).contains(&n)
    {
        return Some(Input::DigitalIn(n));
    }
    None
}

fn parse_mp(rest: &str) -> Option<Input> {
    let (body, soft) = rest
        .strip_suffix("_soft")
        .map_or((rest, false), |b| (b, true));
    let (pos_str, dir_str) = split_pos_dir(body)?;
    Some(Input::Mouthpiece {
        pos: MpPosition::from_csv(pos_str)?,
        dir: SipPuff::from_csv(dir_str)?,
        soft,
    })
}

fn split_pos_dir(body: &str) -> Option<(&str, &str)> {
    for dir in ["sip", "puff"] {
        if let Some(pos) = body.strip_suffix(dir).and_then(|s| s.strip_suffix('_')) {
            return Some((pos, dir));
        }
    }
    None
}

fn parse_side(rest: &str) -> Option<Input> {
    let (body, kind) = strip_side_suffix(rest);
    let dir = SipPuff::from_csv(body)?;
    Some(Input::Side { dir, kind })
}

fn strip_side_suffix(rest: &str) -> (&str, SideKind) {
    for (suffix, kind) in [("_long", SideKind::Long), ("_soft", SideKind::Soft)] {
        if let Some(body) = rest.strip_suffix(suffix) {
            return (body, kind);
        }
    }
    (rest, SideKind::Hard)
}

fn parse_dpad(s: &str) -> Option<(DPadDir, bool)> {
    let (body, inner) = s.strip_suffix("_inner").map_or((s, false), |b| (b, true));
    DPadDir::from_csv(body).map(|d| (d, inner))
}

fn parse_usb_host(host: UsbHost, rest: &str) -> Option<Input> {
    if let Some(button_str) = rest.strip_prefix("button_") {
        let n = button_str.parse::<u8>().ok()?;
        if (1..=15).contains(&n) {
            return Some(Input::UsbHostButton { host, button: n });
        }
        return None;
    }
    if let Some(ax) = JoyAxis::from_csv(rest) {
        return Some(Input::UsbHostAxis { host, axis: ax });
    }
    let (body, inner) = rest
        .strip_suffix("_inner")
        .map_or((rest, false), |b| (b, true));
    if let Some(dir) = DPadDir::from_csv(body) {
        return Some(Input::UsbHostDpad { host, dir, inner });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mp_position_round_trips_for_every_variant() {
        for pos in MpPosition::ALL {
            let id = pos.as_csv();
            let back = MpPosition::from_csv(id).expect(id);
            assert_eq!(*pos, back, "round-trip failed for {id}");
        }
    }

    #[test]
    fn sip_puff_csv_tokens() {
        assert_eq!(SipPuff::Sip.as_csv(), "sip");
        assert_eq!(SipPuff::Puff.as_csv(), "puff");
        assert_eq!(SipPuff::from_csv("sip"), Some(SipPuff::Sip));
    }

    #[test]
    fn joy_axis_round_trips() {
        for ax in JoyAxis::ALL {
            assert_eq!(JoyAxis::from_csv(ax.as_csv()), Some(*ax));
        }
    }

    #[test]
    fn dpad_dir_covers_eight_directions() {
        assert_eq!(DPadDir::ALL.len(), 8);
        for d in DPadDir::ALL {
            assert_eq!(DPadDir::from_csv(d.as_csv()), Some(*d));
        }
    }

    #[test]
    fn usb_host_is_two_hosts() {
        assert_eq!(UsbHost::ALL, &[UsbHost::One, UsbHost::Two]);
        assert_eq!(UsbHost::One.as_csv_index(), "1");
        assert_eq!(UsbHost::Two.as_csv_index(), "2");
    }
}

#[cfg(test)]
mod input_enum_tests {
    use super::*;

    // Every documented input identifier from the QuadStick user manual.
    // source: https://quadstick.s3.amazonaws.com/documents/user_manual/um/dropdown_list_for_inputs.htm
    const ALL_INPUT_IDS: &[&str] = &[
        // Mouthpiece — hard
        "mp_left_sip",
        "mp_left_puff",
        "mp_center_sip",
        "mp_center_puff",
        "mp_right_sip",
        "mp_right_puff",
        "mp_left_center_sip",
        "mp_left_center_puff",
        "mp_right_center_sip",
        "mp_right_center_puff",
        "mp_left_right_sip",
        "mp_left_right_puff",
        "mp_triple_sip",
        "mp_triple_puff",
        // Mouthpiece — soft
        "mp_left_sip_soft",
        "mp_left_puff_soft",
        "mp_center_sip_soft",
        "mp_center_puff_soft",
        "mp_right_sip_soft",
        "mp_right_puff_soft",
        "mp_left_center_sip_soft",
        "mp_left_center_puff_soft",
        "mp_right_center_sip_soft",
        "mp_right_center_puff_soft",
        "mp_left_right_sip_soft",
        "mp_left_right_puff_soft",
        "mp_triple_sip_soft",
        "mp_triple_puff_soft",
        // Side tube
        "right_sip",
        "right_puff",
        "right_sip_soft",
        "right_puff_soft",
        "right_sip_long",
        "right_puff_long",
        // Lip
        "lip",
        "lip_soft",
        // Joystick analog
        "left",
        "right",
        "up",
        "down",
        "any_direction",
        // Joystick D-pad zones (outer)
        "N",
        "NE",
        "E",
        "SE",
        "S",
        "SW",
        "W",
        "NW",
        // Joystick D-pad zones (inner)
        "N_inner",
        "NE_inner",
        "E_inner",
        "SE_inner",
        "S_inner",
        "SW_inner",
        "W_inner",
        "NW_inner",
        // USB-A host 1 analog
        "usb_1_left",
        "usb_1_right",
        "usb_1_up",
        "usb_1_down",
        // USB-A host 1 D-pad outer
        "usb_1_N",
        "usb_1_NE",
        "usb_1_E",
        "usb_1_SE",
        "usb_1_S",
        "usb_1_SW",
        "usb_1_W",
        "usb_1_NW",
        // USB-A host 1 D-pad inner
        "usb_1_N_inner",
        "usb_1_NE_inner",
        "usb_1_E_inner",
        "usb_1_SE_inner",
        "usb_1_S_inner",
        "usb_1_SW_inner",
        "usb_1_W_inner",
        "usb_1_NW_inner",
        // USB-A host 1 buttons
        "usb_1_button_1",
        "usb_1_button_2",
        "usb_1_button_3",
        "usb_1_button_4",
        "usb_1_button_5",
        "usb_1_button_6",
        "usb_1_button_7",
        "usb_1_button_8",
        "usb_1_button_9",
        "usb_1_button_10",
        "usb_1_button_11",
        "usb_1_button_12",
        "usb_1_button_13",
        "usb_1_button_14",
        "usb_1_button_15",
        // USB-A host 2 analog
        "usb_2_left",
        "usb_2_right",
        "usb_2_up",
        "usb_2_down",
        // USB-A host 2 D-pad outer
        "usb_2_N",
        "usb_2_NE",
        "usb_2_E",
        "usb_2_SE",
        "usb_2_S",
        "usb_2_SW",
        "usb_2_W",
        "usb_2_NW",
        // USB-A host 2 D-pad inner
        "usb_2_N_inner",
        "usb_2_NE_inner",
        "usb_2_E_inner",
        "usb_2_SE_inner",
        "usb_2_S_inner",
        "usb_2_SW_inner",
        "usb_2_W_inner",
        "usb_2_NW_inner",
        // USB-A host 2 buttons
        "usb_2_button_1",
        "usb_2_button_2",
        "usb_2_button_3",
        "usb_2_button_4",
        "usb_2_button_5",
        "usb_2_button_6",
        "usb_2_button_7",
        "usb_2_button_8",
        "usb_2_button_9",
        "usb_2_button_10",
        "usb_2_button_11",
        "usb_2_button_12",
        "usb_2_button_13",
        "usb_2_button_14",
        "usb_2_button_15",
        // Digital inputs
        "digital_in_1",
        "digital_in_2",
        "digital_in_3",
        "digital_in_4",
        "digital_in_5",
        "digital_in_6",
        "digital_in_7",
        "digital_in_8",
        // Other
        "center",
        "constant",
    ];

    #[test]
    fn every_documented_id_round_trips() {
        for id in ALL_INPUT_IDS {
            let parsed = Input::from_csv(id);
            assert!(
                !matches!(parsed, Input::Unknown(_)),
                "{id} parsed as Unknown"
            );
            let written = parsed.to_csv();
            assert_eq!(written, *id, "{id} did not round-trip (got {written})");
        }
    }

    #[test]
    fn unknown_input_round_trips_verbatim() {
        let parsed = Input::from_csv("mystery_input");
        assert_eq!(parsed, Input::Unknown("mystery_input".into()));
        assert_eq!(parsed.to_csv(), "mystery_input");
    }

    #[test]
    fn mp_long_does_not_exist_for_mouthpiece() {
        assert!(matches!(
            Input::from_csv("mp_center_sip_long"),
            Input::Unknown(_)
        ));
    }
}
