use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeviceVariant {
    Fps,
    Singleton,
}

#[derive(Debug, Clone, Copy)]
pub struct Station {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: StationKind,
}

#[derive(Debug, Clone, Copy)]
pub enum StationKind {
    Mouthpiece,
    Lip,
    Joystick,
    Side,
}

impl DeviceVariant {
    pub const fn stations(self) -> &'static [Station] {
        match self {
            Self::Fps => &[
                Station {
                    id: "mp_left",
                    label: "MP Left",
                    kind: StationKind::Mouthpiece,
                },
                Station {
                    id: "mp_left_center",
                    label: "MP Left-center",
                    kind: StationKind::Mouthpiece,
                },
                Station {
                    id: "mp_center",
                    label: "MP Center",
                    kind: StationKind::Mouthpiece,
                },
                Station {
                    id: "mp_right_center",
                    label: "MP Right-center",
                    kind: StationKind::Mouthpiece,
                },
                Station {
                    id: "mp_right",
                    label: "MP Right",
                    kind: StationKind::Mouthpiece,
                },
                Station {
                    id: "lip",
                    label: "Lip switch",
                    kind: StationKind::Lip,
                },
                Station {
                    id: "joystick",
                    label: "Joystick",
                    kind: StationKind::Joystick,
                },
            ],
            Self::Singleton => &[
                Station {
                    id: "mp_center",
                    label: "Mouthpiece",
                    kind: StationKind::Mouthpiece,
                },
                Station {
                    id: "lip",
                    label: "Lip switch",
                    kind: StationKind::Lip,
                },
                Station {
                    id: "joystick",
                    label: "Joystick",
                    kind: StationKind::Joystick,
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fps_variant_has_seven_stations() {
        let v = DeviceVariant::Fps;
        assert_eq!(v.stations().len(), 7);
        assert!(v.stations().iter().any(|st| st.id == "lip"));
    }

    #[test]
    fn singleton_variant_has_three_stations() {
        let v = DeviceVariant::Singleton;
        assert_eq!(v.stations().len(), 3);
    }
}
