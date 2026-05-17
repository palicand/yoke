#![cfg(target_os = "macos")]

use std::time::Duration;
use yoke_volume::provider::VolumeProvider;
use yoke_volume::state::MountState;
use yoke_volume_macos::MacOsVolumeProvider;

#[test]
fn real_device_state_within_3s() {
    if std::env::var("YOKE_REAL_DEVICE").as_deref() != Ok("1") {
        eprintln!("YOKE_REAL_DEVICE not set; skipping real-device test");
        return;
    }
    let provider = MacOsVolumeProvider::new().expect("provider construction");
    std::thread::sleep(Duration::from_millis(100));
    let state = provider.current_state();
    eprintln!("real-device state: {state:?}");
    assert!(
        !matches!(state, MountState::Absent),
        "expected QuadStick to be detected (set YOKE_REAL_DEVICE=1 only when one is attached)"
    );
}
