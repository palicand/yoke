use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Channel {
    Usb,
    Bluetooth,
}

impl Channel {
    pub const ALL: &'static [Self] = &[Self::Usb, Self::Bluetooth];

    pub fn from_csv(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.eq_ignore_ascii_case("usb") {
            Some(Self::Usb)
        } else if s.eq_ignore_ascii_case("bluetooth") {
            Some(Self::Bluetooth)
        } else {
            None
        }
    }

    pub const fn canonical_csv(self) -> &'static str {
        match self {
            Self::Usb => "usb",
            Self::Bluetooth => "Bluetooth",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_case_insensitive() {
        for s in ["USB", "usb", "Usb"] {
            assert_eq!(Channel::from_csv(s), Some(Channel::Usb));
        }
        for s in ["Bluetooth", "bluetooth", "BlueTooth", "BLUETOOTH"] {
            assert_eq!(Channel::from_csv(s), Some(Channel::Bluetooth));
        }
    }

    #[test]
    fn canonical_csv_matches_template_form() {
        assert_eq!(Channel::Usb.canonical_csv(), "usb");
        assert_eq!(Channel::Bluetooth.canonical_csv(), "Bluetooth");
    }
}
