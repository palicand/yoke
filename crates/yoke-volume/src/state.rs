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
    DeviceModeChanged {
        vid_pid: VidPid,
        mode_hint: Option<ModeHint>,
    },
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
    Ds3Emulation,
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

// QuadStick re-enumerates under genuine Sony VID:PIDs when a profile activates
// DualShock-style emulation (impersonating a real PS3 controller to satisfy
// hardware checks). Observed at the same physical USB locationID as the base
// "Quad Stick" device, so the device has not actually been unplugged.
pub const QUADSTICK_DS3_EMULATION_VID_PIDS: &[VidPid] = &[
    VidPid {
        vendor: 0x054C,
        product: 0x05C5,
    },
    VidPid {
        vendor: 0x054C,
        product: 0x0268,
    },
];

#[must_use]
pub fn state_transition_event(old: &MountState, new: &MountState) -> Option<MountEvent> {
    match (old, new) {
        (
            MountState::Absent | MountState::DeviceVisibleNoVolume { .. },
            MountState::Present {
                mount_point,
                vid_pid,
                label,
            },
        ) => Some(MountEvent::VolumeMounted {
            mount_point: mount_point.clone(),
            vid_pid: *vid_pid,
            label: label.clone(),
        }),
        (
            MountState::Present { .. },
            MountState::Absent | MountState::DeviceVisibleNoVolume { .. },
        ) => Some(MountEvent::VolumeUnmounted),
        (MountState::Absent, MountState::DeviceVisibleNoVolume { vid_pid, .. }) => {
            Some(MountEvent::DeviceAppeared { vid_pid: *vid_pid })
        }
        (MountState::DeviceVisibleNoVolume { .. }, MountState::Absent) => {
            Some(MountEvent::DeviceDisappeared)
        }
        (
            MountState::DeviceVisibleNoVolume { .. },
            MountState::DeviceVisibleNoVolume { vid_pid, mode_hint },
        ) => Some(MountEvent::DeviceModeChanged {
            vid_pid: *vid_pid,
            mode_hint: *mode_hint,
        }),
        _ => None,
    }
}

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
    fn device_visible_no_volume_to_emulation_emits_mode_changed() {
        let old = MountState::DeviceVisibleNoVolume {
            vid_pid: VidPid {
                vendor: 0x16D0,
                product: 0x092B,
            },
            mode_hint: Some(ModeHint::MassStorageDisabled),
        };
        let new = MountState::DeviceVisibleNoVolume {
            vid_pid: VidPid {
                vendor: 0x054C,
                product: 0x05C5,
            },
            mode_hint: Some(ModeHint::Ds3Emulation),
        };
        let evt = state_transition_event(&old, &new);
        assert!(matches!(
            evt,
            Some(MountEvent::DeviceModeChanged {
                mode_hint: Some(ModeHint::Ds3Emulation),
                ..
            })
        ));
    }

    #[test]
    fn ds3_emulation_pids_disjoint_from_quadstick_set() {
        for vp in QUADSTICK_DS3_EMULATION_VID_PIDS {
            assert!(!QUADSTICK_VID_PIDS.contains(vp));
            assert_ne!(*vp, HORI_PS4_VID_PID);
        }
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
