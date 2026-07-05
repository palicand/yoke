use std::collections::HashSet;
use std::path::PathBuf;
use yoke_volume::state::{HORI_PS4_VID_PID, ModeHint, MountState, VidPid};

#[derive(Default)]
pub struct Tracked {
    pub quadstick_vid_pids: HashSet<VidPid>,
    // A volume devnode whose USB ancestor is a QuadStick exists, even if the
    // filesystem is not readable yet. Distinguishes a mid-mount device
    // ("connecting") from mass-storage-off, like the macOS BSD-name set.
    pub volume_devnode_seen: bool,
    pub hori_seen: bool,
    pub emulation_vp: Option<VidPid>,
    // Sticky across rescans: DEVPKEY_Device_LocationPaths of the last
    // confirmed QuadStick. Recognizes the device after it re-enumerates
    // under an emulation persona we don't have explicitly listed.
    pub quadstick_location: Option<String>,
    pub mount_point: Option<PathBuf>,
    pub label: Option<String>,
}

impl Tracked {
    #[must_use]
    pub fn compute(&self) -> MountState {
        if let Some(vp) = self.quadstick_vid_pids.iter().next().copied() {
            if let (Some(mp), Some(lbl)) = (self.mount_point.as_ref(), self.label.as_ref()) {
                return MountState::Present {
                    mount_point: mp.clone(),
                    vid_pid: vp,
                    label: lbl.clone(),
                };
            }
            if self.volume_devnode_seen {
                return MountState::Mounting { vid_pid: vp };
            }
            return MountState::DeviceVisibleNoVolume {
                vid_pid: vp,
                mode_hint: Some(ModeHint::MassStorageDisabled),
            };
        }
        if let Some(vp) = self.emulation_vp {
            if let (Some(mp), Some(lbl)) = (self.mount_point.as_ref(), self.label.as_ref()) {
                return MountState::Present {
                    mount_point: mp.clone(),
                    vid_pid: vp,
                    label: lbl.clone(),
                };
            }
            return MountState::DeviceVisibleNoVolume {
                vid_pid: vp,
                mode_hint: Some(ModeHint::Emulation),
            };
        }
        if self.hori_seen {
            return MountState::DeviceVisibleNoVolume {
                vid_pid: HORI_PS4_VID_PID,
                mode_hint: Some(ModeHint::Ps4OrHori),
            };
        }
        MountState::Absent
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use yoke_volume::state::{HORI_PS4_VID_PID, ModeHint, MountState};

    const QS: VidPid = VidPid {
        vendor: 0x16D0,
        product: 0x092B,
    };
    const DS3: VidPid = VidPid {
        vendor: 0x054C,
        product: 0x05C5,
    };

    fn with_quadstick() -> Tracked {
        let mut t = Tracked::default();
        t.quadstick_vid_pids.insert(QS);
        t
    }

    #[test]
    fn empty_is_absent() {
        assert_eq!(Tracked::default().compute(), MountState::Absent);
    }

    #[test]
    fn quadstick_with_mount_is_present() {
        let mut t = with_quadstick();
        t.mount_point = Some(PathBuf::from(r"E:\"));
        t.label = Some("QUADSTICK".into());
        assert_eq!(
            t.compute(),
            MountState::Present {
                mount_point: PathBuf::from(r"E:\"),
                vid_pid: QS,
                label: "QUADSTICK".into(),
            }
        );
    }

    #[test]
    fn quadstick_with_volume_devnode_but_no_mount_is_mounting() {
        let mut t = with_quadstick();
        t.volume_devnode_seen = true;
        assert_eq!(t.compute(), MountState::Mounting { vid_pid: QS });
    }

    #[test]
    fn quadstick_without_volume_devnode_is_mass_storage_off() {
        let t = with_quadstick();
        assert_eq!(
            t.compute(),
            MountState::DeviceVisibleNoVolume {
                vid_pid: QS,
                mode_hint: Some(ModeHint::MassStorageDisabled),
            }
        );
    }

    #[test]
    fn emulation_persona_without_mount() {
        let t = Tracked {
            emulation_vp: Some(DS3),
            ..Default::default()
        };
        assert_eq!(
            t.compute(),
            MountState::DeviceVisibleNoVolume {
                vid_pid: DS3,
                mode_hint: Some(ModeHint::Emulation),
            }
        );
    }

    #[test]
    fn emulation_persona_with_mount_is_present() {
        let t = Tracked {
            emulation_vp: Some(DS3),
            mount_point: Some(PathBuf::from(r"E:\")),
            label: Some("QUADSTICK".into()),
            ..Default::default()
        };
        assert_eq!(
            t.compute(),
            MountState::Present {
                mount_point: PathBuf::from(r"E:\"),
                vid_pid: DS3,
                label: "QUADSTICK".into(),
            }
        );
    }

    #[test]
    fn hori_is_ps4_hint() {
        let t = Tracked {
            hori_seen: true,
            ..Default::default()
        };
        assert_eq!(
            t.compute(),
            MountState::DeviceVisibleNoVolume {
                vid_pid: HORI_PS4_VID_PID,
                mode_hint: Some(ModeHint::Ps4OrHori),
            }
        );
    }

    #[test]
    fn quadstick_wins_over_hori_and_emulation() {
        let mut qs = HashSet::new();
        qs.insert(QS);
        let t = Tracked {
            quadstick_vid_pids: qs,
            hori_seen: true,
            emulation_vp: Some(DS3),
            ..Default::default()
        };
        assert!(matches!(
            t.compute(),
            MountState::DeviceVisibleNoVolume {
                vid_pid: QS,
                mode_hint: Some(ModeHint::MassStorageDisabled),
            }
        ));
    }
}
