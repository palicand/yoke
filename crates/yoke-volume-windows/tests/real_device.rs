#![cfg(windows)]

use std::time::Duration;
use yoke_volume::provider::VolumeProvider;
use yoke_volume::state::MountState;
use yoke_volume_windows::WindowsVolumeProvider;

#[test]
fn real_device_state_within_3s() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();
    if std::env::var("YOKE_REAL_DEVICE").as_deref() != Ok("1") {
        tracing::warn!("YOKE_REAL_DEVICE not set; skipping real-device test");
        return;
    }
    let provider = WindowsVolumeProvider::new().expect("provider construction");
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let mut state = provider.current_state();
    while matches!(state, MountState::Absent) && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
        state = provider.current_state();
    }
    tracing::info!(?state, "real-device state");
    assert!(
        !matches!(state, MountState::Absent),
        "expected a device to be detected (set YOKE_REAL_DEVICE=1 only when a QuadStick or YOKE_TEST_VIDPIDS stand-in is attached)"
    );
}
