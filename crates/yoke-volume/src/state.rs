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
    // QuadStick is impersonating a third-party controller (Sony, Xbox,
    // Switch, etc.) so the FAT volume and command channel are not
    // exposed. The specific VID:PID identifies which persona is active.
    Emulation,
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

#[must_use]
pub fn state_transition_events(old: &MountState, new: &MountState) -> Vec<MountEvent> {
    match (old, new) {
        (
            MountState::Absent | MountState::DeviceVisibleNoVolume { .. },
            MountState::Present {
                mount_point,
                vid_pid,
                label,
            },
        ) => vec![MountEvent::VolumeMounted {
            mount_point: mount_point.clone(),
            vid_pid: *vid_pid,
            label: label.clone(),
        }],
        // Volume goes away AND the device persona changes (e.g. mass-storage
        // off + DS3 emulation on). Emit both so a consumer wired only to
        // events still learns the new vid_pid / mode_hint.
        (MountState::Present { .. }, MountState::DeviceVisibleNoVolume { vid_pid, mode_hint }) => {
            vec![
                MountEvent::VolumeUnmounted,
                MountEvent::DeviceModeChanged {
                    vid_pid: *vid_pid,
                    mode_hint: *mode_hint,
                },
            ]
        }
        (MountState::Present { .. }, MountState::Absent) => vec![MountEvent::VolumeUnmounted],
        (MountState::Absent, MountState::DeviceVisibleNoVolume { vid_pid, .. }) => {
            vec![MountEvent::DeviceAppeared { vid_pid: *vid_pid }]
        }
        (MountState::DeviceVisibleNoVolume { .. }, MountState::Absent) => {
            vec![MountEvent::DeviceDisappeared]
        }
        (
            MountState::DeviceVisibleNoVolume {
                vid_pid: old_vid_pid,
                mode_hint: old_mode_hint,
            },
            MountState::DeviceVisibleNoVolume { vid_pid, mode_hint },
        ) if old_vid_pid != vid_pid || old_mode_hint != mode_hint => {
            vec![MountEvent::DeviceModeChanged {
                vid_pid: *vid_pid,
                mode_hint: *mode_hint,
            }]
        }
        _ => Vec::new(),
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
            mode_hint: Some(ModeHint::Emulation),
        };
        let events = state_transition_events(&old, &new);
        assert!(matches!(
            events.as_slice(),
            [MountEvent::DeviceModeChanged {
                mode_hint: Some(ModeHint::Emulation),
                ..
            }]
        ));
    }

    #[test]
    fn identical_device_visible_no_volume_emits_no_event() {
        let s = MountState::DeviceVisibleNoVolume {
            vid_pid: VidPid {
                vendor: 0x16D0,
                product: 0x092B,
            },
            mode_hint: Some(ModeHint::MassStorageDisabled),
        };
        assert!(state_transition_events(&s, &s).is_empty());
    }

    #[test]
    fn present_to_device_visible_emits_unmount_and_mode_change() {
        let old = MountState::Present {
            mount_point: PathBuf::from("/Volumes/Quad Stick"),
            vid_pid: VidPid {
                vendor: 0x16D0,
                product: 0x092B,
            },
            label: "Quad Stick".to_string(),
        };
        let new = MountState::DeviceVisibleNoVolume {
            vid_pid: VidPid {
                vendor: 0x054C,
                product: 0x05C5,
            },
            mode_hint: Some(ModeHint::Emulation),
        };
        let events = state_transition_events(&old, &new);
        assert!(matches!(
            events.as_slice(),
            [
                MountEvent::VolumeUnmounted,
                MountEvent::DeviceModeChanged {
                    mode_hint: Some(ModeHint::Emulation),
                    ..
                },
            ]
        ));
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
