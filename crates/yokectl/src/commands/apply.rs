use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use yoke_edit::EditOp;
use yoke_volume::VolumeProvider;

use crate::error::CliError;
use crate::output::Output;
use crate::target::Target;

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
    let target_obj = Target::classify(target);
    let edits_source = Target::classify(edits.to_str().unwrap_or(""));
    if matches!(target_obj, Target::Stdin) && matches!(edits_source, Target::Stdin) {
        return Err(CliError::StdinConflict.into());
    }
    let bytes = edits_source.read_bytes(provider.as_ref())?;
    let parsed: EditsFile =
        serde_json::from_slice(&bytes).map_err(|e| CliError::MalformedEdits {
            message: e.to_string(),
        })?;
    if dry_run {
        let profile_bytes = target_obj.read_bytes(provider.as_ref())?;
        let pr = yoke_config::parse(&profile_bytes)?;
        yoke_edit::apply(pr.model, &parsed.edits).map_err(anyhow::Error::from)?;
        return out.emit(
            &serde_json::json!({"action": "apply-dry-run", "ops": parsed.edits.len()}),
            |w| writeln!(w, "dry-run ok ({} ops)", parsed.edits.len()),
        );
    }
    crate::commands::edit::load_apply_save(provider.as_ref(), target, &parsed.edits)?;
    out.emit(&serde_json::json!({"action": "apply"}), |w| {
        writeln!(w, "applied")
    })
}
