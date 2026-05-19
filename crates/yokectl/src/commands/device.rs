use std::io::Write;
use std::sync::Arc;

use anyhow::Result;
use yoke_volume::VolumeProvider;
use yoke_volume::state::{MountState, VidPid};

use crate::output::Output;

pub fn run_device(provider: &Arc<dyn VolumeProvider>, out: &Output) -> Result<()> {
    let state = provider.current_state();
    out.emit(&serde_json::json!({"state": state_to_json(&state)}), |w| {
        writeln!(w, "{}", state_human(&state))
    })
}

pub fn run_debug(provider: &Arc<dyn VolumeProvider>, out: &Output) -> Result<()> {
    let state = provider.current_state();
    let entries: Vec<_> = provider.list_profiles().unwrap_or_else(|e| {
        tracing::warn!(error = %e, "list_profiles failed in debug; reporting empty");
        Vec::new()
    });
    out.emit(
        &serde_json::json!({
            "device": state_to_json(&state),
            "mount": state_to_json(&state),
            "profiles": entries.iter().map(|e| serde_json::json!({
                "name": e.name.as_filename(),
                "kind": format!("{:?}", e.kind),
                "byte_len": e.byte_len,
            })).collect::<Vec<_>>(),
        }),
        |w| {
            writeln!(w, "device: {}", state_human(&state))?;
            writeln!(w, "profiles: {}", entries.len())
        },
    )
}

fn state_to_json(s: &MountState) -> serde_json::Value {
    match s {
        MountState::Absent => serde_json::json!({"kind": "Absent"}),
        MountState::DeviceVisibleNoVolume { vid_pid, mode_hint } => serde_json::json!({
            "kind": "DeviceVisibleNoVolume",
            "vid_pid": vid_pid_json(*vid_pid),
            "mode_hint": mode_hint.as_ref().map(|h| format!("{h:?}")),
        }),
        MountState::Present {
            mount_point,
            vid_pid,
            label,
        } => serde_json::json!({
            "kind": "Present",
            "mount_point": mount_point.to_string_lossy(),
            "vid_pid": vid_pid_json(*vid_pid),
            "label": label,
        }),
    }
}

fn vid_pid_json(v: VidPid) -> serde_json::Value {
    serde_json::json!({"vendor": v.vendor, "product": v.product})
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
