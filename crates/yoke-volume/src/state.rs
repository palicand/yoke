use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MountState {
    Absent,
    DeviceVisibleNoVolume {
        vid_pid: VidPid,
        mode_hint: Option<ModeHint>,
    },
    Present {
        mount_point: PathBuf,
        vid_pid: VidPid,
        label: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MountEvent {
    DeviceAppeared {
        vid_pid: VidPid,
    },
    DeviceDisappeared,
    VolumeMounted {
        mount_point: PathBuf,
        vid_pid: VidPid,
        label: String,
    },
    VolumeUnmounted,
    MultipleDevicesDetected {
        count: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct VidPid {
    pub vendor: u16,
    pub product: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ModeHint {
    Ps4OrHori,
    MassStorageDisabled,
}

pub const QUADSTICK_VID_PIDS: &[VidPid] = &[
    VidPid {
        vendor: 0x16D0,
        product: 0x092B,
    },
    VidPid {
        vendor: 0x16D0,
        product: 0x092C,
    },
    VidPid {
        vendor: 0x16D0,
        product: 0x092D,
    },
    VidPid {
        vendor: 0x16D0,
        product: 0x092E,
    },
    VidPid {
        vendor: 0x1FC9,
        product: 0x205B,
    },
];

pub const HORI_PS4_VID_PID: VidPid = VidPid {
    vendor: 0x0F0D,
    product: 0x0066,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quadstick_vid_pids_contains_primary() {
        let primary = VidPid {
            vendor: 0x16D0,
            product: 0x092B,
        };
        assert!(QUADSTICK_VID_PIDS.contains(&primary));
    }

    #[test]
    fn hori_vid_pid_is_not_in_quadstick_set() {
        assert!(!QUADSTICK_VID_PIDS.contains(&HORI_PS4_VID_PID));
    }

    #[test]
    fn mount_state_serde_round_trip() {
        let state = MountState::Present {
            mount_point: PathBuf::from("/Volumes/Quad Stick"),
            vid_pid: VidPid {
                vendor: 0x16D0,
                product: 0x092B,
            },
            label: "Quad Stick".to_string(),
        };
        let json = serde_json::to_string(&state).unwrap();
        let restored: MountState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, restored);
    }

    #[test]
    fn mount_event_serde_round_trip() {
        let evt = MountEvent::VolumeMounted {
            mount_point: PathBuf::from("/Volumes/Quad Stick"),
            vid_pid: VidPid {
                vendor: 0x16D0,
                product: 0x092B,
            },
            label: "Quad Stick".to_string(),
        };
        let json = serde_json::to_string(&evt).unwrap();
        let restored: MountEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(evt, restored);
    }
}
