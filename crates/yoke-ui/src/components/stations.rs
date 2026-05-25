//! Static layout for the device map's eight stations plus the
//! `input_belongs_to` lookup used by the bindings panel filter.
//!
//! Coordinates live in this module rather than CSS so the SVG `<svg>` element
//! can size itself purely from the constant table. There is a separate
//! `StationKind` in `yoke_config::catalog::variants` describing the device-side
//! station model; the kind here is UI-only and stays decoupled because the map
//! sketch is a fixed visual whose shape does not change per device variant.

use yoke_config::catalog::{Input, MpPosition};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StationKind {
    Joystick,
    Mouthpiece,
    Lip,
    Side,
}

#[derive(Debug, Clone, Copy)]
pub struct StationDef {
    pub id: &'static str,
    pub label: &'static str,
    pub short: &'static str,
    pub kind: StationKind,
    pub x: f32,
    pub y: f32,
}

// Layout positions mirror design_handoff_quadstick_config/src/data.js and
// device.jsx Map layout: five mouthpiece positions in a row, joystick on top,
// lip below center, side tube on the right.
pub const STATIONS: &[StationDef] = &[
    StationDef {
        id: "joystick",
        label: "Joystick",
        short: "J",
        kind: StationKind::Joystick,
        x: 140.0,
        y: 42.0,
    },
    StationDef {
        id: "mp_left",
        label: "MP Left",
        short: "L",
        kind: StationKind::Mouthpiece,
        x: 42.0,
        y: 110.0,
    },
    StationDef {
        id: "mp_left_center",
        label: "MP Left-C",
        short: "LC",
        kind: StationKind::Mouthpiece,
        x: 78.0,
        y: 110.0,
    },
    StationDef {
        id: "mp_center",
        label: "MP Center",
        short: "C",
        kind: StationKind::Mouthpiece,
        x: 114.0,
        y: 110.0,
    },
    StationDef {
        id: "mp_right_center",
        label: "MP Right-C",
        short: "RC",
        kind: StationKind::Mouthpiece,
        x: 150.0,
        y: 110.0,
    },
    StationDef {
        id: "mp_right",
        label: "MP Right",
        short: "R",
        kind: StationKind::Mouthpiece,
        x: 186.0,
        y: 110.0,
    },
    StationDef {
        id: "lip",
        label: "Lip switch",
        short: "Lip",
        kind: StationKind::Lip,
        x: 140.0,
        y: 172.0,
    },
    StationDef {
        id: "side",
        label: "Side tube",
        short: "S",
        kind: StationKind::Side,
        x: 242.0,
        y: 120.0,
    },
];

#[must_use]
pub fn find(id: &str) -> Option<&'static StationDef> {
    STATIONS.iter().find(|s| s.id == id)
}

/// True when the given binding's input belongs to the named station.
///
/// Maps the design-handoff station ids to `Input` enum variants from
/// `yoke_config`. Multi-position mouthpiece bindings (`LeftRight`, `Triple`)
/// do not belong to any single station and return `false`; the bindings panel
/// shows them only in its "ALL" view.
#[must_use]
pub fn input_belongs_to(input: &Input, station_id: &str) -> bool {
    let Some(station) = find(station_id) else {
        return false;
    };
    match (station.kind, input) {
        (
            StationKind::Joystick,
            Input::JoystickAxis(_) | Input::JoystickDpad { .. } | Input::JoystickAnyDirection,
        )
        | (StationKind::Lip, Input::Lip { .. })
        | (StationKind::Side, Input::Side { .. }) => true,
        (StationKind::Mouthpiece, Input::Mouthpiece { pos, .. }) => {
            mp_position_id(*pos).is_some_and(|id| id == station.id)
        }
        _ => false,
    }
}

const fn mp_position_id(pos: MpPosition) -> Option<&'static str> {
    match pos {
        MpPosition::Left => Some("mp_left"),
        MpPosition::LeftCenter => Some("mp_left_center"),
        MpPosition::Center => Some("mp_center"),
        MpPosition::RightCenter => Some("mp_right_center"),
        MpPosition::Right => Some("mp_right"),
        // LeftRight and Triple are multi-position bindings — they don't map to
        // a single station, so they're invisible to per-station filtering.
        MpPosition::LeftRight | MpPosition::Triple => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yoke_config::catalog::inputs::SideKind;
    use yoke_config::catalog::{JoyAxis, SipPuff};

    #[test]
    fn eight_stations_with_unique_ids() {
        assert_eq!(STATIONS.len(), 8);
        let mut ids: Vec<_> = STATIONS.iter().map(|s| s.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 8, "station ids must be unique");
    }

    #[test]
    fn find_returns_known_station() {
        assert_eq!(find("joystick").map(|s| s.label), Some("Joystick"));
        assert!(find("bogus").is_none());
    }

    #[test]
    fn joystick_inputs_belong_to_joystick() {
        assert!(input_belongs_to(
            &Input::JoystickAxis(JoyAxis::Left),
            "joystick",
        ));
        assert!(input_belongs_to(&Input::JoystickAnyDirection, "joystick"));
        assert!(!input_belongs_to(
            &Input::JoystickAxis(JoyAxis::Left),
            "lip",
        ));
    }

    #[test]
    fn mouthpiece_positions_route_to_their_stations() {
        let input = Input::Mouthpiece {
            pos: MpPosition::LeftCenter,
            dir: SipPuff::Sip,
            soft: false,
        };
        assert!(input_belongs_to(&input, "mp_left_center"));
        assert!(!input_belongs_to(&input, "mp_center"));
    }

    #[test]
    fn multi_position_mouthpiece_belongs_to_no_station() {
        for pos in [MpPosition::LeftRight, MpPosition::Triple] {
            let input = Input::Mouthpiece {
                pos,
                dir: SipPuff::Puff,
                soft: false,
            };
            for station in STATIONS {
                assert!(
                    !input_belongs_to(&input, station.id),
                    "{pos:?} unexpectedly matched station {}",
                    station.id,
                );
            }
        }
    }

    #[test]
    fn side_input_belongs_to_side_station() {
        let input = Input::Side {
            dir: SipPuff::Sip,
            kind: SideKind::Hard,
        };
        assert!(input_belongs_to(&input, "side"));
        assert!(!input_belongs_to(&input, "joystick"));
    }

    #[test]
    fn lip_input_belongs_to_lip_station() {
        assert!(input_belongs_to(&Input::Lip { soft: false }, "lip"));
        assert!(input_belongs_to(&Input::Lip { soft: true }, "lip"));
        assert!(!input_belongs_to(&Input::Lip { soft: false }, "side"));
    }
}
