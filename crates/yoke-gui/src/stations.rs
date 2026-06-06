use yoke_config::catalog::{Input, MpPosition};
use yoke_config::model::SubProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StationKind {
    Mouthpiece,
    Lip,
    Joystick,
    Side,
}

#[derive(Debug, Clone, Copy)]
pub struct Station {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: StationKind,
    pub x: f32,
    pub y: f32,
}

pub const VIEWBOX_W: f32 = 100.0;
pub const VIEWBOX_H: f32 = 80.0;

pub const FPS_STATIONS: &[Station] = &[
    Station {
        id: "joystick",
        label: "Joystick",
        kind: StationKind::Joystick,
        x: 50.0,
        y: 22.0,
    },
    Station {
        id: "mp_left",
        label: "MP Left",
        kind: StationKind::Mouthpiece,
        x: 22.0,
        y: 48.0,
    },
    Station {
        id: "mp_left_center",
        label: "MP Left-center",
        kind: StationKind::Mouthpiece,
        x: 36.0,
        y: 48.0,
    },
    Station {
        id: "mp_center",
        label: "MP Center",
        kind: StationKind::Mouthpiece,
        x: 50.0,
        y: 48.0,
    },
    Station {
        id: "mp_right_center",
        label: "MP Right-center",
        kind: StationKind::Mouthpiece,
        x: 64.0,
        y: 48.0,
    },
    Station {
        id: "mp_right",
        label: "MP Right",
        kind: StationKind::Mouthpiece,
        x: 78.0,
        y: 48.0,
    },
    Station {
        id: "lip",
        label: "Lip switch",
        kind: StationKind::Lip,
        x: 50.0,
        y: 66.0,
    },
    Station {
        id: "side",
        label: "Side bypass tube",
        kind: StationKind::Side,
        x: 88.0,
        y: 34.0,
    },
];

#[must_use]
pub fn station_by_id(id: &str) -> Option<&'static Station> {
    FPS_STATIONS.iter().find(|s| s.id == id)
}

/// Map a physical `Input` to the device-map station it lives on, if any.
///
/// Combined mouthpiece positions (`LeftRight`, `Triple`), `Constant`, USB-host
/// inputs, digital ins, and `Unknown` have no single station and return `None`
/// (they appear only in the unfiltered bindings view).
#[must_use]
pub const fn input_belongs_to(input: &Input) -> Option<&'static str> {
    match input {
        Input::Mouthpiece { pos, .. } => match pos {
            MpPosition::Left => Some("mp_left"),
            MpPosition::LeftCenter => Some("mp_left_center"),
            MpPosition::Center => Some("mp_center"),
            MpPosition::RightCenter => Some("mp_right_center"),
            MpPosition::Right => Some("mp_right"),
            MpPosition::LeftRight | MpPosition::Triple => None,
        },
        Input::Side { .. } => Some("side"),
        Input::Lip { .. } => Some("lip"),
        Input::JoystickAxis(_)
        | Input::JoystickDpad { .. }
        | Input::JoystickAnyDirection
        | Input::Center => Some("joystick"),
        Input::Constant
        | Input::UsbHostAxis { .. }
        | Input::UsbHostDpad { .. }
        | Input::UsbHostButton { .. }
        | Input::DigitalIn(_)
        | Input::Unknown(_) => None,
    }
}

/// Physical inputs belonging to a station, in catalog order.
#[must_use]
pub fn station_inputs(station: &str) -> Vec<String> {
    Input::all_csv_names()
        .filter(|name| input_belongs_to(&Input::from_csv(name)) == Some(station))
        .collect()
}

/// Count bindings per station id for one sub-profile.
#[must_use]
pub fn binding_counts(sub: &SubProfile) -> std::collections::HashMap<&'static str, usize> {
    let mut counts = std::collections::HashMap::new();
    for b in sub.bindings() {
        if let Some(input) = &b.input
            && let Some(station) = input_belongs_to(input)
        {
            *counts.entry(station).or_insert(0) += 1;
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use yoke_config::catalog::inputs::SideKind;
    use yoke_config::catalog::{JoyAxis, SipPuff};

    #[test]
    fn station_inputs_enumerates_lip_inputs() {
        let inputs = station_inputs("lip");
        assert!(inputs.contains(&"lip".to_string()));
        assert!(inputs.contains(&"lip_soft".to_string()));
        assert!(
            inputs
                .iter()
                .all(|i| { input_belongs_to(&Input::from_csv(i)) == Some("lip") })
        );
    }

    #[test]
    fn station_inputs_unknown_station_is_empty() {
        assert!(station_inputs("nonexistent").is_empty());
    }

    #[test]
    fn fps_has_eight_stations_including_side() {
        assert_eq!(FPS_STATIONS.len(), 8);
        assert!(FPS_STATIONS.iter().any(|s| s.id == "side"));
    }

    #[test]
    fn all_station_coords_are_within_viewbox() {
        for s in FPS_STATIONS {
            assert!((0.0..=VIEWBOX_W).contains(&s.x), "{} x out of range", s.id);
            assert!((0.0..=VIEWBOX_H).contains(&s.y), "{} y out of range", s.id);
        }
    }

    #[test]
    fn mouthpiece_positions_map_to_their_stations() {
        let cases = [
            (MpPosition::Left, "mp_left"),
            (MpPosition::LeftCenter, "mp_left_center"),
            (MpPosition::Center, "mp_center"),
            (MpPosition::RightCenter, "mp_right_center"),
            (MpPosition::Right, "mp_right"),
        ];
        for (pos, station) in cases {
            let input = Input::Mouthpiece {
                pos,
                dir: SipPuff::Sip,
                soft: false,
            };
            assert_eq!(input_belongs_to(&input), Some(station));
        }
    }

    #[test]
    fn combined_mouthpiece_positions_have_no_station() {
        let input = Input::Mouthpiece {
            pos: MpPosition::LeftRight,
            dir: SipPuff::Puff,
            soft: false,
        };
        assert_eq!(input_belongs_to(&input), None);
    }

    #[test]
    fn side_lip_and_joystick_map_to_their_stations() {
        assert_eq!(
            input_belongs_to(&Input::Side {
                dir: SipPuff::Sip,
                kind: SideKind::Hard
            }),
            Some("side")
        );
        assert_eq!(input_belongs_to(&Input::Lip { soft: true }), Some("lip"));
        assert_eq!(
            input_belongs_to(&Input::JoystickAxis(JoyAxis::Up)),
            Some("joystick")
        );
        assert_eq!(
            input_belongs_to(&Input::JoystickAnyDirection),
            Some("joystick")
        );
        assert_eq!(input_belongs_to(&Input::Center), Some("joystick"));
    }

    #[test]
    fn binding_counts_attribute_to_stations() {
        // CRLF endings required; layout mirrors SINGLE_SUB in yoke-config parse tests.
        // "left" -> JoystickAxis(Left) -> "joystick"; "lip" -> Lip -> "lip".
        let csv = b"QuadStick Configuration,Version 1.4,,Mac\r\n\
Profile Name,,Mouse Mode,\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mouse_left,normal,left,\r\n\
kb_left_shift,normal,lip,\r\n\
\r\n";
        let parsed = yoke_config::parse(csv).unwrap();
        let sub = &parsed.model.sub_profiles[0];
        let counts = binding_counts(sub);
        assert_eq!(counts.get("joystick").copied().unwrap_or(0), 1);
        assert_eq!(counts.get("lip").copied().unwrap_or(0), 1);
    }

    #[test]
    fn unmapped_inputs_return_none() {
        assert_eq!(input_belongs_to(&Input::Constant), None);
        assert_eq!(input_belongs_to(&Input::DigitalIn(3)), None);
        assert_eq!(input_belongs_to(&Input::Unknown("xyz".into())), None);
    }
}
