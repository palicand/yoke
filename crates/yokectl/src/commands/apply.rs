use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use yoke_edit::EditOp;
use yoke_volume::VolumeProvider;

use crate::output::Output;

#[derive(serde::Deserialize)]
struct EditsFile {
    edits: Vec<EditOp>,
}

pub fn run(
    provider: &Arc<dyn VolumeProvider>,
    out: &Output,
    target: &str,
    edits: &Path,
    dry_run: bool,
) -> Result<()> {
    let json = if edits == Path::new("-") {
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s)?;
        s
    } else {
        std::fs::read_to_string(edits).with_context(|| format!("read {}", edits.display()))?
    };
    let parsed: EditsFile = serde_json::from_str(&json)?;
    if dry_run {
        let target_obj = crate::target::Target::classify(target);
        let bytes = target_obj.read_bytes(provider.as_ref())?;
        let pr = yoke_config::parse(&bytes)?;
        let _updated = yoke_edit::apply(pr.model, &parsed.edits).map_err(anyhow::Error::from)?;
        out.emit(
            &serde_json::json!({"action": "apply-dry-run", "ops": parsed.edits.len()}),
            |w| writeln!(w, "dry-run ok ({} ops)", parsed.edits.len()),
        )?;
        return Ok(());
    }
    crate::commands::edit::load_apply_save(provider.as_ref(), target, &parsed.edits)?;
    out.emit(&serde_json::json!({"action": "apply"}), |w| {
        writeln!(w, "applied")
    })
}
