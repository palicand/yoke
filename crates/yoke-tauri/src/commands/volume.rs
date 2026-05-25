use tauri::State;
use yoke_ipc::VolumePresence;
use yoke_volume::state::{ModeHint, MountState};

use crate::AppState;

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command signature requires State by value"
)]
pub fn volume_state(state: State<'_, AppState>) -> VolumePresence {
    presence_from(&state.volume.current_state())
}

fn presence_from(state: &MountState) -> VolumePresence {
    match state {
        MountState::Absent => VolumePresence::Absent,
        MountState::DeviceVisibleNoVolume { mode_hint, .. } => {
            VolumePresence::DeviceVisibleNoVolume {
                mode_hint: mode_hint.map(mode_hint_label),
            }
        }
        MountState::Present {
            label, mount_point, ..
        } => VolumePresence::Present {
            label: label.clone(),
            mount_point: mount_point.clone(),
        },
    }
}

fn mode_hint_label(hint: ModeHint) -> String {
    match hint {
        ModeHint::Ps4OrHori => "Ps4OrHori",
        ModeHint::MassStorageDisabled => "MassStorageDisabled",
        ModeHint::Emulation => "Emulation",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use yoke_volume::state::VidPid;

    #[test]
    fn absent_maps_to_absent() {
        assert_eq!(presence_from(&MountState::Absent), VolumePresence::Absent);
    }

    #[test]
    fn present_carries_label_and_mount() {
        let s = MountState::Present {
            mount_point: PathBuf::from("/Volumes/Quad"),
            vid_pid: VidPid {
                vendor: 0x16D0,
                product: 0x092B,
            },
            label: "Quad".into(),
        };
        assert_eq!(
            presence_from(&s),
            VolumePresence::Present {
                label: "Quad".into(),
                mount_point: PathBuf::from("/Volumes/Quad"),
            }
        );
    }

    #[test]
    fn device_visible_carries_hint() {
        let s = MountState::DeviceVisibleNoVolume {
            vid_pid: VidPid {
                vendor: 0x16D0,
                product: 0x092B,
            },
            mode_hint: Some(ModeHint::Emulation),
        };
        assert_eq!(
            presence_from(&s),
            VolumePresence::DeviceVisibleNoVolume {
                mode_hint: Some("Emulation".into()),
            }
        );
    }

    #[test]
    fn mode_hint_label_covers_all_variants() {
        for (hint, expected) in [
            (ModeHint::Ps4OrHori, "Ps4OrHori"),
            (ModeHint::MassStorageDisabled, "MassStorageDisabled"),
            (ModeHint::Emulation, "Emulation"),
        ] {
            let s = MountState::DeviceVisibleNoVolume {
                vid_pid: VidPid {
                    vendor: 0x16D0,
                    product: 0x092B,
                },
                mode_hint: Some(hint),
            };
            assert_eq!(
                presence_from(&s),
                VolumePresence::DeviceVisibleNoVolume {
                    mode_hint: Some(expected.into()),
                },
                "mode_hint_label mismatch for {hint:?}",
            );
        }
    }

    #[test]
    fn device_visible_without_hint_maps_to_none() {
        let s = MountState::DeviceVisibleNoVolume {
            vid_pid: VidPid {
                vendor: 0x16D0,
                product: 0x092B,
            },
            mode_hint: None,
        };
        assert_eq!(
            presence_from(&s),
            VolumePresence::DeviceVisibleNoVolume { mode_hint: None },
        );
    }
}
