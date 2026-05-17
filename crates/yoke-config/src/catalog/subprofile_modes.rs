use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SubProfileMode {
    Mouse,
    MouseScroll,
    LeftAnalog,
    RightAnalog,
    MixedAnalog,
    DPad,
    Unknown(String),
}

const TABLE: &[(&str, &str)] = &[
    ("Mouse", "Mouse Mode"),
    ("Mouse Scroll", ""),
    ("Left Analog", "Left joy"),
    ("Right Analog", "Right joy"),
    ("Mixed Analog", "Mixed Joystick"),
    ("D-Pad", ""),
];

impl SubProfileMode {
    pub const KNOWN: &'static [Self] = &[
        Self::Mouse,
        Self::MouseScroll,
        Self::LeftAnalog,
        Self::RightAnalog,
        Self::MixedAnalog,
        Self::DPad,
    ];

    const fn idx(&self) -> Option<usize> {
        match self {
            Self::Mouse => Some(0),
            Self::MouseScroll => Some(1),
            Self::LeftAnalog => Some(2),
            Self::RightAnalog => Some(3),
            Self::MixedAnalog => Some(4),
            Self::DPad => Some(5),
            Self::Unknown(_) => None,
        }
    }

    pub fn canonical_csv(&self) -> String {
        self.idx().map_or_else(
            || match self {
                Self::Unknown(s) => s.clone(),
                _ => unreachable!(),
            },
            |i| TABLE[i].0.to_owned(),
        )
    }

    pub fn from_csv(s: &str) -> Option<Self> {
        if s.is_empty() {
            return None;
        }
        let trimmed = s.trim();
        for (i, (canon, syn)) in TABLE.iter().enumerate() {
            if trimmed == *canon || (!syn.is_empty() && trimmed == *syn) {
                return Some(Self::KNOWN[i].clone());
            }
        }
        Some(Self::Unknown(trimmed.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_names_parse() {
        for (s, expected) in [
            ("Mouse", SubProfileMode::Mouse),
            ("Mouse Scroll", SubProfileMode::MouseScroll),
            ("Left Analog", SubProfileMode::LeftAnalog),
            ("Right Analog", SubProfileMode::RightAnalog),
            ("Mixed Analog", SubProfileMode::MixedAnalog),
            ("D-Pad", SubProfileMode::DPad),
        ] {
            assert_eq!(SubProfileMode::from_csv(s), Some(expected));
        }
    }

    #[test]
    fn legacy_synonyms_parse_to_same_variant() {
        assert_eq!(
            SubProfileMode::from_csv("Mouse Mode"),
            Some(SubProfileMode::Mouse)
        );
        assert_eq!(
            SubProfileMode::from_csv("Left joy"),
            Some(SubProfileMode::LeftAnalog)
        );
        assert_eq!(
            SubProfileMode::from_csv("Right joy"),
            Some(SubProfileMode::RightAnalog)
        );
        assert_eq!(
            SubProfileMode::from_csv("Mixed Joystick"),
            Some(SubProfileMode::MixedAnalog)
        );
    }

    #[test]
    fn canonical_to_csv_is_current_template_form() {
        assert_eq!(SubProfileMode::LeftAnalog.canonical_csv(), "Left Analog");
        assert_eq!(SubProfileMode::Mouse.canonical_csv(), "Mouse");
    }

    #[test]
    fn unknown_mode_round_trips() {
        let m = SubProfileMode::from_csv("Future Mode").unwrap();
        assert_eq!(m, SubProfileMode::Unknown("Future Mode".into()));
    }
}
