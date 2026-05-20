use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use yoke_volume::VolumeProvider;
use yoke_volume::profile::ProfileName;

use crate::output::Output;

pub fn run_list(provider: &Arc<dyn VolumeProvider>, out: &Output) -> Result<()> {
    let entries = provider.list_profiles()?;
    out.emit(
        &serde_json::json!({
            "profiles": entries.iter().map(|e| serde_json::json!({
                "name": e.name.stem(),
                "kind": e.kind,
                "byte_len": e.byte_len,
            })).collect::<Vec<_>>(),
        }),
        |w| {
            for e in &entries {
                writeln!(w, "{} ({:?}, {} bytes)", e.name.stem(), e.kind, e.byte_len)?;
            }
            Ok(())
        },
    )
}

pub fn run_show(
    provider: &Arc<dyn VolumeProvider>,
    out: &Output,
    target: &str,
    raw: bool,
) -> Result<()> {
    let t = crate::target::Target::classify(target);
    let bytes = t.read_bytes(provider.as_ref())?;
    if raw {
        std::io::stdout().write_all(&bytes)?;
        return Ok(());
    }
    let parsed = yoke_config::parse(&bytes).context("parsing profile")?;
    let p = parsed.model;
    out.emit(
        &serde_json::json!({
            "title": p.top_line.title,
            "sub_profiles": p.sub_profiles.iter().map(|sp| serde_json::json!({
                "name": sp.header.profile_name,
                "mode": sp.header.mode.canonical_csv(),
                "channel": sp.header.channel.canonical_csv(),
                "bindings": sp.bindings().count(),
                "overrides": sp.overrides().count(),
            })).collect::<Vec<_>>(),
            "preferences": p.preferences.as_ref().map_or(0, |pf| pf.entries.len()),
        }),
        |w| {
            writeln!(w, "title: {}", p.top_line.title)?;
            for sp in &p.sub_profiles {
                writeln!(
                    w,
                    "  sub-profile {}: {:?} / {:?}",
                    sp.header.profile_name, sp.header.mode, sp.header.channel
                )?;
            }
            Ok(())
        },
    )
}

pub fn run_validate(provider: &Arc<dyn VolumeProvider>, out: &Output, target: &str) -> Result<()> {
    let t = crate::target::Target::classify(target);
    let bytes = t.read_bytes(provider.as_ref())?;
    let parsed = yoke_config::parse(&bytes)?;
    out.emit(
        &serde_json::json!({
            "warnings": parsed.warnings.iter().map(|w| format!("{w:?}")).collect::<Vec<_>>(),
        }),
        |w| writeln!(w, "ok ({} warnings)", parsed.warnings.len()),
    )
}

pub fn run_pull(
    provider: &Arc<dyn VolumeProvider>,
    out: &Output,
    name: &str,
    dest: Option<PathBuf>,
) -> Result<()> {
    let pn = ProfileName::new(name)?;
    let bytes = provider.read_profile(&pn)?;
    let target = dest.unwrap_or_else(|| PathBuf::from(pn.as_filename()));
    std::fs::write(&target, &bytes).with_context(|| format!("write {}", target.display()))?;
    out.emit(
        &serde_json::json!({
            "pulled": pn.stem(),
            "dest": target.display().to_string(),
            "bytes": bytes.len(),
        }),
        |w| writeln!(w, "pulled {} -> {}", pn.stem(), target.display()),
    )
}

pub fn run_push(
    provider: &Arc<dyn VolumeProvider>,
    out: &Output,
    src: &Path,
    name: Option<&str>,
    validate: bool,
) -> Result<()> {
    let bytes = std::fs::read(src).with_context(|| format!("read {}", src.display()))?;
    if validate {
        yoke_config::parse(&bytes)?;
    }
    let stem = name.map_or_else(
        || {
            src.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("profile")
                .to_string()
        },
        str::to_string,
    );
    let pn = ProfileName::new(&stem)?;
    provider.write_profile(&pn, &bytes)?;
    out.emit(
        &serde_json::json!({
            "pushed": pn.stem(),
            "bytes": bytes.len(),
        }),
        |w| writeln!(w, "pushed {} ({} bytes)", pn.stem(), bytes.len()),
    )
}

pub fn run_copy(
    provider: &Arc<dyn VolumeProvider>,
    out: &Output,
    from: &str,
    to: &str,
) -> Result<()> {
    let fr = ProfileName::new(from)?;
    let to_n = ProfileName::new(to)?;
    let bytes = provider.read_profile(&fr)?;
    provider.write_profile(&to_n, &bytes)?;
    out.emit(
        &serde_json::json!({"copied": {"from": fr.stem(), "to": to_n.stem()}}),
        |w| writeln!(w, "copied {} -> {}", fr.stem(), to_n.stem()),
    )
}

pub fn run_rename(
    provider: &Arc<dyn VolumeProvider>,
    out: &Output,
    from: &str,
    to: &str,
) -> Result<()> {
    let fr = ProfileName::new(from)?;
    let to_n = ProfileName::new(to)?;
    provider.rename_profile(&fr, &to_n)?;
    out.emit(
        &serde_json::json!({"renamed": {"from": fr.stem(), "to": to_n.stem()}}),
        |w| writeln!(w, "renamed {} -> {}", fr.stem(), to_n.stem()),
    )
}

pub fn run_delete(
    provider: &Arc<dyn VolumeProvider>,
    out: &Output,
    name: &str,
    force: bool,
) -> Result<()> {
    let pn = ProfileName::new(name)?;
    if !force {
        anyhow::bail!("refusing to delete {} without --force", pn.stem());
    }
    provider.delete_profile(&pn)?;
    out.emit(&serde_json::json!({"deleted": pn.stem()}), |w| {
        writeln!(w, "deleted {}", pn.stem())
    })
}
