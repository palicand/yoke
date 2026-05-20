use std::io::Write;
use std::sync::Arc;

use anyhow::Result;
use yoke_config::catalog::{Channel, SubProfileMode};
use yoke_edit::EditOp;
use yoke_volume::VolumeProvider;

use crate::output::Output;

fn parse_mode(s: &str) -> Result<SubProfileMode> {
    let parsed = SubProfileMode::from_csv(s)
        .ok_or_else(|| anyhow::anyhow!("unknown sub-profile mode: {s}"))?;
    if matches!(parsed, SubProfileMode::Unknown(_)) {
        anyhow::bail!("unknown sub-profile mode: {s}");
    }
    Ok(parsed)
}

fn parse_channel(s: &str) -> Result<Channel> {
    Channel::from_csv(s).ok_or_else(|| anyhow::anyhow!("unknown channel: {s}"))
}

pub fn run_add(
    provider: &Arc<dyn VolumeProvider>,
    out: &Output,
    target: &str,
    name: &str,
    mode: &str,
    channel: &str,
    sub_mode: Option<&str>,
) -> Result<()> {
    let op = EditOp::AddSubProfile {
        name: name.to_string(),
        mode: parse_mode(mode)?,
        sub_mode: sub_mode.unwrap_or("").to_string(),
        channel: parse_channel(channel)?,
    };
    crate::commands::edit::load_apply_save(provider.as_ref(), target, &[op])?;
    out.emit(
        &serde_json::json!({"action": "subprofile-add", "name": name}),
        |w| writeln!(w, "added {name}"),
    )
}

pub fn run_delete(
    provider: &Arc<dyn VolumeProvider>,
    out: &Output,
    target: &str,
    name: &str,
) -> Result<()> {
    crate::commands::edit::load_apply_save(
        provider.as_ref(),
        target,
        &[EditOp::DeleteSubProfile {
            name: name.to_string(),
        }],
    )?;
    out.emit(
        &serde_json::json!({"action": "subprofile-delete", "name": name}),
        |w| writeln!(w, "deleted {name}"),
    )
}

pub fn run_rename(
    provider: &Arc<dyn VolumeProvider>,
    out: &Output,
    target: &str,
    from: &str,
    to: &str,
) -> Result<()> {
    crate::commands::edit::load_apply_save(
        provider.as_ref(),
        target,
        &[EditOp::RenameSubProfile {
            from: from.to_string(),
            to: to.to_string(),
        }],
    )?;
    out.emit(&serde_json::json!({"action": "subprofile-rename"}), |w| {
        writeln!(w, "renamed {from} -> {to}")
    })
}

pub fn run_clone(
    provider: &Arc<dyn VolumeProvider>,
    out: &Output,
    target: &str,
    from: &str,
    to: &str,
) -> Result<()> {
    crate::commands::edit::load_apply_save(
        provider.as_ref(),
        target,
        &[EditOp::CloneSubProfile {
            from: from.to_string(),
            to: to.to_string(),
        }],
    )?;
    out.emit(&serde_json::json!({"action": "subprofile-clone"}), |w| {
        writeln!(w, "cloned {from} -> {to}")
    })
}
