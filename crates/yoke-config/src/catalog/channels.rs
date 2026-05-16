use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Channel {
    Usb,
    Bluetooth,
}

impl Channel {
    pub fn from_csv(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "usb" => Some(Self::Usb),
            "bluetooth" => Some(Self::Bluetooth),
            _ => None,
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
