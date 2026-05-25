use std::path::PathBuf;
use tauri::State;
use yoke_config::parse;
use yoke_ipc::{BackendError, DeviceProfileEntry, Profile};
use yoke_volume::ProfileName;

use crate::AppState;

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command signature requires State by value"
)]
pub fn list_device_profiles(
    state: State<'_, AppState>,
) -> Result<Vec<DeviceProfileEntry>, BackendError> {
    let entries = state
        .volume
        .list_profiles()
        .map_err(|e| BackendError::Io(e.to_string()))?;
    Ok(entries
        .into_iter()
        .map(|e| DeviceProfileEntry {
            name: e.name.as_filename().to_string(),
            kind: format!("{:?}", e.kind),
        })
        .collect())
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
    let profile_name =
        ProfileName::new(&name).map_err(|e| BackendError::NotFound(e.to_string()))?;
    let bytes = state
        .volume
        .read_profile(&profile_name)
        .map_err(|e| BackendError::Io(e.to_string()))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use yoke_volume::FsBackend;

    fn fixture_state() -> AppState {
        let tmp = tempfile::tempdir().unwrap();
        let fs = FsBackend::new(tmp.path().to_path_buf());
        fs.set_present(true);
        std::fs::write(
            tmp.path().join("default.csv"),
            include_str!("../../../yoke-config/tests/fixtures/minimal_profile.csv"),
        )
        .unwrap();
        // Leak the tempdir so the path stays valid for the test duration —
        // VolumeProvider holds the path, not the TempDir handle, so dropping
        // the handle would unlink the directory underneath it.
        std::mem::forget(tmp);
        AppState {
            volume: std::sync::Arc::new(fs),
        }
    }

    #[test]
    fn list_returns_fixture_profile() {
        // Constructing a real tauri::State in a unit test is awkward, so we
        // exercise the underlying provider logic directly — the command body
        // is just a translation layer on top of list_profiles.
        let state = fixture_state();
        let entries = state.volume.list_profiles().unwrap();
        assert!(
            entries
                .iter()
                .any(|e| e.name.as_filename() == "default.csv")
        );
    }

    #[test]
    fn read_file_profile_parses_real_csv() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../yoke-config/tests/fixtures/minimal_profile.csv");
        let result = read_file_profile(path).unwrap();
        assert!(!result.sub_profiles.is_empty());
    }
}
