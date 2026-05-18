use std::io::Write;
use std::sync::Arc;

use anyhow::{Context, Result};
use yoke_edit::{EditOp, PreferenceValue, apply};
use yoke_volume::VolumeProvider;

use crate::output::Output;

fn parse_pref_value(raw: &str) -> PreferenceValue {
    if let Ok(n) = raw.parse::<i64>() {
        return PreferenceValue::Number(n);
    }
    match raw.to_ascii_lowercase().as_str() {
        "true" => PreferenceValue::Bool(true),
        "false" => PreferenceValue::Bool(false),
        _ => PreferenceValue::Text(raw.to_string()),
    }
}

pub fn load_apply_save(
    provider: &dyn VolumeProvider,
    target_str: &str,
    ops: &[EditOp],
) -> Result<()> {
    let target = crate::target::Target::classify(target_str);
    let bytes = target.read_bytes(provider)?;
    let parsed = yoke_config::parse(&bytes).context("parse profile")?;
    let updated = apply(parsed.model, ops).map_err(anyhow::Error::from)?;
    // Template-fidelity write preserves byte layout when the model still matches the template;
    // add/delete sub-profile ops change the section count, so fall back to canonical.
    let out_bytes = match yoke_config::write(&updated, Some(&parsed.raw)) {
        Ok(b) => b,
        Err(yoke_config::WriteError::InvariantViolation(_)) => {
            tracing::warn!(
                "template invalidated by structural edit; falling back to canonical layout"
            );
            yoke_config::write(&updated, None)?
        }
    };
    target.write_bytes(provider, &out_bytes)?;
    Ok(())
}

pub fn run_set_title(
    provider: &Arc<dyn VolumeProvider>,
    out: &Output,
    target: &str,
    title: &str,
) -> Result<()> {
    load_apply_save(
        provider.as_ref(),
        target,
        &[EditOp::SetTitle {
            title: title.to_string(),
        }],
    )?;
    out.emit(
        &serde_json::json!({"action": "set-title", "title": title}),
        |w| writeln!(w, "title set to {title}"),
    )
}

pub fn run_set_preference(
    provider: &Arc<dyn VolumeProvider>,
    out: &Output,
    target: &str,
    key: &str,
    value: &str,
) -> Result<()> {
    load_apply_save(
        provider.as_ref(),
        target,
        &[EditOp::SetPreference {
            key: key.to_string(),
            value: parse_pref_value(value),
        }],
    )?;
    out.emit(
        &serde_json::json!({"action": "set-preference", "key": key, "value": value}),
        |w| writeln!(w, "preference {key} = {value}"),
    )
}

pub fn run_unset_preference(
    provider: &Arc<dyn VolumeProvider>,
    out: &Output,
    target: &str,
    key: &str,
) -> Result<()> {
    load_apply_save(
        provider.as_ref(),
        target,
        &[EditOp::UnsetPreference {
            key: key.to_string(),
        }],
    )?;
    out.emit(
        &serde_json::json!({"action": "unset-preference", "key": key}),
        |w| writeln!(w, "preference {key} cleared"),
    )
}

pub fn run_set_override(
    provider: &Arc<dyn VolumeProvider>,
    out: &Output,
    target: &str,
    sp: &str,
    key: &str,
    value: &str,
) -> Result<()> {
    load_apply_save(
        provider.as_ref(),
        target,
        &[EditOp::SetOverride {
            sub_profile: sp.to_string(),
            key: key.to_string(),
            value: parse_pref_value(value),
        }],
    )?;
    out.emit(
        &serde_json::json!({"action": "set-override", "sub_profile": sp, "key": key}),
        |w| writeln!(w, "override {sp}.{key} = {value}"),
    )
}

pub fn run_unset_override(
    provider: &Arc<dyn VolumeProvider>,
    out: &Output,
    target: &str,
    sp: &str,
    key: &str,
) -> Result<()> {
    load_apply_save(
        provider.as_ref(),
        target,
        &[EditOp::UnsetOverride {
            sub_profile: sp.to_string(),
            key: key.to_string(),
        }],
    )?;
    out.emit(
        &serde_json::json!({"action": "unset-override", "sub_profile": sp, "key": key}),
        |w| writeln!(w, "override {sp}.{key} cleared"),
    )
}

pub fn run_set_binding(
    provider: &Arc<dyn VolumeProvider>,
    out: &Output,
    target: &str,
    sp: &str,
    input: &str,
    output_s: &str,
) -> Result<()> {
    load_apply_save(
        provider.as_ref(),
        target,
        &[EditOp::SetBinding {
            sub_profile: sp.to_string(),
            input: input.to_string(),
            output: output_s.to_string(),
        }],
    )?;
    out.emit(&serde_json::json!({"action": "set-binding"}), |w| {
        writeln!(w, "binding set")
    })
}

pub fn run_clear_binding(
    provider: &Arc<dyn VolumeProvider>,
    out: &Output,
    target: &str,
    sp: &str,
    input: &str,
) -> Result<()> {
    load_apply_save(
        provider.as_ref(),
        target,
        &[EditOp::ClearBinding {
            sub_profile: sp.to_string(),
            input: input.to_string(),
        }],
    )?;
    out.emit(&serde_json::json!({"action": "clear-binding"}), |w| {
        writeln!(w, "binding cleared")
    })
}
