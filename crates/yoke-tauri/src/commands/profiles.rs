use std::path::PathBuf;
use tauri::State;
use tauri_plugin_dialog::DialogExt;
use yoke_config::parse;
use yoke_ipc::{BackendError, DeviceProfileEntry, Profile};
use yoke_volume::{ProfileEntry, ProfileName, VolumeError};

use crate::AppState;

fn map_volume_err(e: VolumeError) -> BackendError {
    match e {
        VolumeError::NotPresent | VolumeError::VolumeHidden { .. } => {
            BackendError::VolumeNotPresent
        }
        VolumeError::InvalidProfileName(name) => BackendError::NotFound(name),
        VolumeError::BackendInit(detail) => BackendError::NotInitialized(detail),
        VolumeError::Io(err) => BackendError::Io(err.to_string()),
    }
}

fn entries_to_dto(entries: Vec<ProfileEntry>) -> Vec<DeviceProfileEntry> {
    entries
        .into_iter()
        .map(|e| DeviceProfileEntry {
            name: e.name.as_filename().to_string(),
            kind: format!("{:?}", e.kind),
        })
        .collect()
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command signature requires State by value"
)]
pub fn list_device_profiles(
    state: State<'_, AppState>,
) -> Result<Vec<DeviceProfileEntry>, BackendError> {
    let entries = state.volume.list_profiles().map_err(map_volume_err)?;
    Ok(entries_to_dto(entries))
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command signature requires State by value"
)]
pub fn read_device_profile(
    name: String,
    state: State<'_, AppState>,
) -> Result<Profile, BackendError> {
    let profile_name = ProfileName::new(&name).map_err(map_volume_err)?;
    let bytes = state
        .volume
        .read_profile(&profile_name)
        .map_err(map_volume_err)?;
    let parsed = parse(&bytes).map_err(|e| BackendError::Parse(e.to_string()))?;
    Ok(parsed.model)
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command parameters must be owned for IPC deserialization"
)]
pub fn read_file_profile(path: PathBuf) -> Result<Profile, BackendError> {
    let bytes = std::fs::read(&path).map_err(|e| BackendError::Io(e.to_string()))?;
    let parsed = parse(&bytes).map_err(|e| BackendError::Parse(e.to_string()))?;
    Ok(parsed.model)
}

#[tauri::command]
pub async fn pick_file_dialog(app: tauri::AppHandle) -> Option<PathBuf> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("QuadStick profile", &["csv"])
        .pick_file(move |path| {
            let _ = tx.send(path.and_then(|p| p.into_path().ok()));
        });
    rx.await.ok().flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;
    use yoke_volume::ProfileKind;

    fn entry(name: &str, kind: ProfileKind) -> ProfileEntry {
        ProfileEntry {
            name: ProfileName::new(name).unwrap(),
            kind,
            byte_len: 0,
            modified: SystemTime::UNIX_EPOCH,
        }
    }

    #[test]
    fn entries_to_dto_stringifies_each_kind() {
        let input = vec![
            entry("default", ProfileKind::Default),
            entry("prefs", ProfileKind::Prefs),
            entry("destiny", ProfileKind::Game),
        ];
        let dto = entries_to_dto(input);
        assert_eq!(
            dto,
            vec![
                DeviceProfileEntry {
                    name: "default.csv".into(),
                    kind: "Default".into(),
                },
                DeviceProfileEntry {
                    name: "prefs.csv".into(),
                    kind: "Prefs".into(),
                },
                DeviceProfileEntry {
                    name: "destiny.csv".into(),
                    kind: "Game".into(),
                },
            ],
        );
    }

    #[test]
    fn entries_to_dto_handles_empty() {
        assert!(entries_to_dto(vec![]).is_empty());
    }

    #[test]
    fn map_volume_err_not_present_to_volume_not_present() {
        assert_eq!(
            map_volume_err(VolumeError::NotPresent),
            BackendError::VolumeNotPresent,
        );
    }

    #[test]
    fn map_volume_err_volume_hidden_to_volume_not_present() {
        assert_eq!(
            map_volume_err(VolumeError::VolumeHidden { hint: None }),
            BackendError::VolumeNotPresent,
        );
        assert_eq!(
            map_volume_err(VolumeError::VolumeHidden {
                hint: Some(yoke_volume::state::ModeHint::Emulation),
            }),
            BackendError::VolumeNotPresent,
        );
    }

    #[test]
    fn map_volume_err_invalid_name_to_not_found() {
        let mapped = map_volume_err(VolumeError::InvalidProfileName("foo/bar".into()));
        assert_eq!(mapped, BackendError::NotFound("foo/bar".into()));
    }

    #[test]
    fn map_volume_err_backend_init_to_not_initialized() {
        let mapped = map_volume_err(VolumeError::BackendInit("disk arb dead".into()));
        assert_eq!(mapped, BackendError::NotInitialized("disk arb dead".into()),);
    }

    #[test]
    fn map_volume_err_io_preserves_display() {
        let io = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "nope");
        let mapped = map_volume_err(VolumeError::Io(io));
        let BackendError::Io(detail) = mapped else {
            panic!("expected BackendError::Io");
        };
        assert!(detail.contains("nope"), "detail was {detail:?}");
    }

    #[test]
    fn read_file_profile_parses_real_csv() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../yoke-config/tests/fixtures/minimal_profile.csv");
        let result = read_file_profile(path).unwrap();
        assert!(!result.sub_profiles.is_empty());
    }
}
