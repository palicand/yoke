use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PreferenceKey {
    Known(KnownPreference),
    Unknown(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KnownPreference {
    SipPuffThreshold,
    SipPuffThresholdSoft,
    SipPuffMaximum,
    SipPuffDelaySoft,
    LipPositionMinimum,
    LipPositionMaximum,
    JoystickDeflectionMinimum,
    JoystickDeflectionMaximum,
    JoystickDeadZoneShape,
    AntiDeadZone,
    JoystickWarning,
    JoystickAlarm,
    JoystickDpadInner,
    JoystickDpadOuter,
    DeflectionMultiplierUp,
    DeflectionMultiplierDown,
    DeflectionMultiplierLeft,
    DeflectionMultiplierRight,
    Usb1MultiplierUp,
    Usb1MultiplierDown,
    Usb1MultiplierLeft,
    Usb1MultiplierRight,
    Usb2MultiplierUp,
    Usb2MultiplierDown,
    Usb2MultiplierLeft,
    Usb2MultiplierRight,
    MouseSpeed,
    MouseResponseCurve,
    Volume,
    Brightness,
    DigitalOut1,
    DigitalOut2,
    BluetoothDeviceMode,
    BluetoothAuthenticationMode,
    BluetoothConnectionMode,
    BluetoothThrottle,
    BluetoothRemoteAddress,
    EnableSwapInputs,
    EnableSelectFiles,
    EnableUsbAdevice,
    EnableDs3Emulation,
    EnableRumble,
    EnableUsbComm,
    Debug,
    WatchdogDisable,
}

#[derive(Debug, Clone, Copy)]
pub enum PreferenceValueKind {
    IntRange { min: i32, max: i32 },
    Bool,
    Select(&'static [&'static str]),
    SelectInt(&'static [i32]),
    Text,
}

#[derive(Debug, Clone, Copy)]
pub struct PreferenceSpec {
    pub id: &'static str,
    pub key: KnownPreference,
    pub kind: PreferenceValueKind,
    pub default: &'static str,
    pub label: &'static str,
}

impl PreferenceSpec {
    // source: https://quadstick.s3.amazonaws.com/documents/user_manual/um/preferences.htm
    // reconciled against per-config overrides observed in the canonical corpus.
    pub const ALL: &'static [Self] = &[
        Self {
            id: "sip_puff_threshold",
            key: KnownPreference::SipPuffThreshold,
            kind: PreferenceValueKind::IntRange { min: 0, max: 100 },
            default: "40",
            label: "Hard threshold",
        },
        Self {
            id: "sip_puff_threshold_soft",
            key: KnownPreference::SipPuffThresholdSoft,
            kind: PreferenceValueKind::IntRange { min: 0, max: 100 },
            default: "8",
            label: "Soft threshold",
        },
        Self {
            id: "sip_puff_maximum",
            key: KnownPreference::SipPuffMaximum,
            kind: PreferenceValueKind::IntRange { min: 0, max: 100 },
            default: "70",
            label: "Saturation",
        },
        Self {
            id: "sip_puff_delay_soft",
            key: KnownPreference::SipPuffDelaySoft,
            kind: PreferenceValueKind::IntRange { min: 0, max: 5000 },
            default: "1000",
            label: "Soft delay",
        },
        Self {
            id: "lip_position_minimum",
            key: KnownPreference::LipPositionMinimum,
            kind: PreferenceValueKind::IntRange { min: 0, max: 100 },
            default: "8",
            label: "Lip engage",
        },
        Self {
            id: "lip_position_maximum",
            key: KnownPreference::LipPositionMaximum,
            kind: PreferenceValueKind::IntRange { min: 0, max: 100 },
            default: "35",
            label: "Lip release",
        },
        Self {
            id: "joystick_deflection_minimum",
            key: KnownPreference::JoystickDeflectionMinimum,
            kind: PreferenceValueKind::IntRange { min: 0, max: 100 },
            default: "9",
            label: "Joystick dead zone",
        },
        Self {
            id: "joystick_deflection_maximum",
            key: KnownPreference::JoystickDeflectionMaximum,
            kind: PreferenceValueKind::IntRange { min: 0, max: 100 },
            default: "30",
            label: "Joystick max",
        },
        Self {
            id: "joystick_dead_zone_shape",
            key: KnownPreference::JoystickDeadZoneShape,
            kind: PreferenceValueKind::SelectInt(&[0, 1]),
            default: "1",
            label: "Dead zone shape",
        },
        Self {
            id: "anti_dead_zone",
            key: KnownPreference::AntiDeadZone,
            kind: PreferenceValueKind::IntRange { min: 0, max: 100 },
            default: "0",
            label: "Anti dead zone",
        },
        Self {
            id: "joystick_warning",
            key: KnownPreference::JoystickWarning,
            kind: PreferenceValueKind::IntRange { min: 0, max: 2000 },
            default: "400",
            label: "Warning",
        },
        Self {
            id: "joystick_alarm",
            key: KnownPreference::JoystickAlarm,
            kind: PreferenceValueKind::IntRange { min: 0, max: 2000 },
            default: "500",
            label: "Alarm",
        },
        Self {
            id: "joystick_D_Pad_inner",
            key: KnownPreference::JoystickDpadInner,
            kind: PreferenceValueKind::IntRange { min: 0, max: 100 },
            default: "25",
            label: "D-Pad inner",
        },
        Self {
            id: "joystick_D_Pad_outer",
            key: KnownPreference::JoystickDpadOuter,
            kind: PreferenceValueKind::IntRange { min: 0, max: 100 },
            default: "60",
            label: "D-Pad outer",
        },
        Self {
            id: "deflection_multiplier_up",
            key: KnownPreference::DeflectionMultiplierUp,
            kind: PreferenceValueKind::IntRange { min: 0, max: 200 },
            default: "140",
            label: "Up",
        },
        Self {
            id: "deflection_multiplier_down",
            key: KnownPreference::DeflectionMultiplierDown,
            kind: PreferenceValueKind::IntRange { min: 0, max: 200 },
            default: "130",
            label: "Down",
        },
        Self {
            id: "deflection_multiplier_left",
            key: KnownPreference::DeflectionMultiplierLeft,
            kind: PreferenceValueKind::IntRange { min: 0, max: 200 },
            default: "100",
            label: "Left",
        },
        Self {
            id: "deflection_multiplier_right",
            key: KnownPreference::DeflectionMultiplierRight,
            kind: PreferenceValueKind::IntRange { min: 0, max: 200 },
            default: "100",
            label: "Right",
        },
        Self {
            id: "usb_1_multiplier_up",
            key: KnownPreference::Usb1MultiplierUp,
            kind: PreferenceValueKind::IntRange { min: 0, max: 200 },
            default: "100",
            label: "USB1 up",
        },
        Self {
            id: "usb_1_multiplier_down",
            key: KnownPreference::Usb1MultiplierDown,
            kind: PreferenceValueKind::IntRange { min: 0, max: 200 },
            default: "100",
            label: "USB1 down",
        },
        Self {
            id: "usb_1_multiplier_left",
            key: KnownPreference::Usb1MultiplierLeft,
            kind: PreferenceValueKind::IntRange { min: 0, max: 200 },
            default: "100",
            label: "USB1 left",
        },
        Self {
            id: "usb_1_multiplier_right",
            key: KnownPreference::Usb1MultiplierRight,
            kind: PreferenceValueKind::IntRange { min: 0, max: 200 },
            default: "100",
            label: "USB1 right",
        },
        Self {
            id: "usb_2_multiplier_up",
            key: KnownPreference::Usb2MultiplierUp,
            kind: PreferenceValueKind::IntRange { min: 0, max: 200 },
            default: "100",
            label: "USB2 up",
        },
        Self {
            id: "usb_2_multiplier_down",
            key: KnownPreference::Usb2MultiplierDown,
            kind: PreferenceValueKind::IntRange { min: 0, max: 200 },
            default: "100",
            label: "USB2 down",
        },
        Self {
            id: "usb_2_multiplier_left",
            key: KnownPreference::Usb2MultiplierLeft,
            kind: PreferenceValueKind::IntRange { min: 0, max: 200 },
            default: "100",
            label: "USB2 left",
        },
        Self {
            id: "usb_2_multiplier_right",
            key: KnownPreference::Usb2MultiplierRight,
            kind: PreferenceValueKind::IntRange { min: 0, max: 200 },
            default: "100",
            label: "USB2 right",
        },
        Self {
            id: "mouse_speed",
            key: KnownPreference::MouseSpeed,
            kind: PreferenceValueKind::IntRange { min: 0, max: 1000 },
            default: "100",
            label: "Mouse speed",
        },
        Self {
            id: "mouse_response_curve",
            key: KnownPreference::MouseResponseCurve,
            kind: PreferenceValueKind::SelectInt(&[0, 1, 2]),
            default: "1",
            label: "Mouse curve",
        },
        Self {
            id: "volume",
            key: KnownPreference::Volume,
            kind: PreferenceValueKind::IntRange { min: 0, max: 100 },
            default: "40",
            label: "Volume",
        },
        Self {
            id: "brightness",
            key: KnownPreference::Brightness,
            kind: PreferenceValueKind::IntRange { min: 0, max: 100 },
            default: "75",
            label: "Brightness",
        },
        Self {
            id: "digital_out_1",
            key: KnownPreference::DigitalOut1,
            kind: PreferenceValueKind::Bool,
            default: "0",
            label: "Relay 1",
        },
        Self {
            id: "digital_out_2",
            key: KnownPreference::DigitalOut2,
            kind: PreferenceValueKind::Bool,
            default: "0",
            label: "Relay 2",
        },
        Self {
            id: "bluetooth_device_mode",
            key: KnownPreference::BluetoothDeviceMode,
            kind: PreferenceValueKind::Select(&["none", "keyboard", "mouse", "gamepad"]),
            default: "none",
            label: "BT device",
        },
        Self {
            id: "bluetooth_authentication_mode",
            key: KnownPreference::BluetoothAuthenticationMode,
            kind: PreferenceValueKind::IntRange { min: 0, max: 6 },
            default: "2",
            label: "BT auth",
        },
        Self {
            id: "bluetooth_connection_mode",
            key: KnownPreference::BluetoothConnectionMode,
            kind: PreferenceValueKind::Select(&["pair", "remember"]),
            default: "pair",
            label: "BT connect",
        },
        Self {
            id: "bluetooth_throttle",
            key: KnownPreference::BluetoothThrottle,
            kind: PreferenceValueKind::IntRange { min: 1, max: 1000 },
            default: "15",
            label: "BT throttle",
        },
        Self {
            id: "bluetooth_remote_address",
            key: KnownPreference::BluetoothRemoteAddress,
            kind: PreferenceValueKind::Text,
            default: "",
            label: "BT remote",
        },
        Self {
            id: "enable_swap_inputs",
            key: KnownPreference::EnableSwapInputs,
            kind: PreferenceValueKind::Bool,
            default: "0",
            label: "Swap inputs",
        },
        Self {
            id: "enable_select_files",
            key: KnownPreference::EnableSelectFiles,
            kind: PreferenceValueKind::Bool,
            default: "1",
            label: "Select files",
        },
        Self {
            id: "enable_usb_a_device",
            key: KnownPreference::EnableUsbAdevice,
            kind: PreferenceValueKind::Bool,
            default: "0",
            label: "USB-A device",
        },
        Self {
            id: "enable_DS3_emulation",
            key: KnownPreference::EnableDs3Emulation,
            kind: PreferenceValueKind::SelectInt(&[0, 1, 2, 3, 4, 5, 6, 7]),
            default: "0",
            label: "DS3 emulation",
        },
        Self {
            id: "enable_rumble",
            key: KnownPreference::EnableRumble,
            kind: PreferenceValueKind::Bool,
            default: "0",
            label: "Rumble",
        },
        Self {
            id: "enable_usb_comm",
            key: KnownPreference::EnableUsbComm,
            kind: PreferenceValueKind::Bool,
            default: "0",
            label: "USB comm",
        },
        Self {
            id: "debug",
            key: KnownPreference::Debug,
            kind: PreferenceValueKind::Bool,
            default: "1",
            label: "Debug",
        },
        Self {
            id: "watchdog_disable",
            key: KnownPreference::WatchdogDisable,
            kind: PreferenceValueKind::Bool,
            default: "0",
            label: "Disable watchdog",
        },
    ];

    pub fn for_id(id: &str) -> Option<Self> {
        Self::ALL.iter().find(|s| s.id == id).copied()
    }

    pub fn for_key(key: &PreferenceKey) -> Option<Self> {
        match key {
            PreferenceKey::Known(k) => Self::ALL.iter().find(|s| s.key == *k).copied(),
            PreferenceKey::Unknown(_) => None,
        }
    }

    pub fn validate(&self, raw: &str) -> Result<(), String> {
        match self.kind {
            PreferenceValueKind::IntRange { min, max } => {
                let v: i32 = raw.parse().map_err(|_| format!("'{raw}' is not an int"))?;
                if v < min || v > max {
                    Err(format!("'{raw}' outside [{min}..{max}]"))
                } else {
                    Ok(())
                }
            }
            PreferenceValueKind::Bool => match raw {
                "0" | "1" => Ok(()),
                _ => Err(format!("'{raw}' is not 0/1")),
            },
            PreferenceValueKind::Select(opts) => {
                if opts.contains(&raw) {
                    Ok(())
                } else {
                    Err(format!("'{raw}' not one of {opts:?}"))
                }
            }
            PreferenceValueKind::SelectInt(opts) => {
                let v: i32 = raw.parse().map_err(|_| format!("'{raw}' is not an int"))?;
                if opts.contains(&v) {
                    Ok(())
                } else {
                    Err(format!("'{raw}' not one of {opts:?}"))
                }
            }
            PreferenceValueKind::Text => Ok(()),
        }
    }
}

impl PreferenceKey {
    /// Parse a non-empty CSV preference identifier. Empty strings are caller-filtered.
    /// Unknown identifiers become `PreferenceKey::Unknown(s)` so they survive round-trip.
    pub fn from_csv(s: &str) -> Self {
        debug_assert!(
            !s.is_empty(),
            "PreferenceKey::from_csv called with empty string"
        );
        PreferenceSpec::for_id(s)
            .map_or_else(|| Self::Unknown(s.to_owned()), |spec| Self::Known(spec.key))
    }

    pub fn as_csv(&self) -> String {
        match self {
            Self::Known(k) => PreferenceSpec::ALL
                .iter()
                .find(|s| s.key == *k)
                .map_or_else(String::new, |s| s.id.to_owned()),
            Self::Unknown(s) => s.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_key_round_trips() {
        for spec in PreferenceSpec::ALL {
            let parsed = PreferenceKey::from_csv(spec.id);
            assert_eq!(parsed.as_csv(), spec.id);
            assert!(!matches!(parsed, PreferenceKey::Unknown(_)));
        }
    }

    #[test]
    fn unknown_key_round_trips() {
        let k = PreferenceKey::from_csv("future_pref");
        assert_eq!(k, PreferenceKey::Unknown("future_pref".into()));
        assert_eq!(k.as_csv(), "future_pref");
    }

    #[test]
    fn every_known_id_has_a_spec() {
        for spec in PreferenceSpec::ALL {
            let key = PreferenceKey::from_csv(spec.id);
            assert_eq!(PreferenceSpec::for_key(&key).map(|s| s.id), Some(spec.id));
        }
    }

    #[test]
    fn int_range_validates() {
        let spec = PreferenceSpec::for_id("sip_puff_threshold").unwrap();
        assert!(spec.validate("40").is_ok());
        assert!(spec.validate("101").is_err());
        assert!(spec.validate("abc").is_err());
    }
}
