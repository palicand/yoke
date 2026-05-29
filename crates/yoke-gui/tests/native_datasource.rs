#![cfg(not(target_arch = "wasm32"))]

use std::sync::Arc;

use yoke_gui::data::DataSource;
use yoke_gui::data::native::NativeDataSource;
use yoke_volume::state::MountState;
use yoke_volume::{FsBackend, ProfileName, VolumeProvider};

// Minimal valid QuadStick CSV: header + one sub-profile with one binding.
// Row layout (blank line separates header from sub-profile section):
//   1. Top-line
//   2. blank
//   3. Profile Name row  (col 2 = mode)
//   4. sub-mode row      (col 2 = sub_mode, first cell blank)
//   5. column header row (col 2 = channel)
//   6. binding row
const SAMPLE: &str = "QuadStick Configuration,Version 1.4,,Mac\r\n\
\r\n\
Profile Name,,Mouse Mode,\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mouse_left,normal,left,\r\n";

#[test]
fn reads_a_device_profile_from_a_temp_volume() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("default.csv"), SAMPLE).unwrap();
    let backend = Arc::new(FsBackend::new(dir.path().to_path_buf()));

    // FsBackend::new over an existing directory returns Present immediately;
    // no set_present call needed.
    assert!(matches!(
        backend.current_state(),
        MountState::Present { .. }
    ));

    let data = NativeDataSource::for_test(backend);
    assert!(matches!(data.volume_state(), MountState::Present { .. }));

    let list = data.list_device_profiles().unwrap();
    assert!(
        list.iter()
            .any(|e| e.name == ProfileName::new("default").unwrap())
    );

    let profile = data
        .read_device_profile(&ProfileName::new("default").unwrap())
        .unwrap();
    assert!(!profile.sub_profiles.is_empty());
}

#[test]
fn reads_a_local_file_profile() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("portal2.csv");
    std::fs::write(&path, SAMPLE).unwrap();
    let backend = Arc::new(FsBackend::new(dir.path().to_path_buf()));
    let data = NativeDataSource::for_test(backend);
    let profile = data.read_file_profile(&path).unwrap();
    assert!(!profile.sub_profiles.is_empty());
}
