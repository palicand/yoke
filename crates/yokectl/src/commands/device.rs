use std::sync::Arc;

use anyhow::Result;
use yoke_volume::VolumeProvider;
use yoke_volume::state::MountState;

use crate::output::Output;

pub fn run_device(provider: &Arc<dyn VolumeProvider>, out: &Output) -> Result<()> {
    let state = provider.current_state();
    out.emit(&serde_json::json!({"state": state}), |w| {
        writeln!(w, "{}", state_human(&state))
    })
}

pub fn run_debug(provider: &Arc<dyn VolumeProvider>, out: &Output) -> Result<()> {
    let state = provider.current_state();
    // debug must always emit a snapshot; degrade to an empty profile list rather than failing.
    let entries: Vec<_> = provider.list_profiles().unwrap_or_else(|e| {
        tracing::warn!(error = %e, "list_profiles failed in debug; reporting empty");
        Vec::new()
    });
    out.emit(
        &serde_json::json!({
            "device": state,
            "profiles": entries.iter().map(super::profile_entry_json).collect::<Vec<_>>(),
        }),
        |w| {
            writeln!(w, "device: {}", state_human(&state))?;
            writeln!(w, "profiles: {}", entries.len())
        },
    )
}

fn state_human(s: &MountState) -> String {
    match s {
        MountState::Absent => "Absent".into(),
        MountState::DeviceVisibleNoVolume { vid_pid, mode_hint } => format!(
            "DeviceVisibleNoVolume vid={:04X}:{:04X} hint={:?}",
            vid_pid.vendor, vid_pid.product, mode_hint
        ),
        MountState::Present {
            mount_point,
            vid_pid,
            label,
        } => format!(
            "Present mount={} label={} vid={:04X}:{:04X}",
            mount_point.display(),
            label,
            vid_pid.vendor,
            vid_pid.product
        ),
    }
}
