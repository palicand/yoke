use std::io::Write;
use std::sync::Arc;

use anyhow::{Context, Result};
use yoke_index::{IndexClient, ProfileSource};
use yoke_volume::VolumeProvider;
use yoke_volume::profile::ProfileName;

use crate::output::Output;
use crate::runtime;

pub fn run(
    provider: &Arc<dyn VolumeProvider>,
    out: &Output,
    source: &str,
    as_name: Option<&str>,
    dry_run: bool,
    no_validate: bool,
) -> Result<()> {
    let src = ProfileSource::classify(source).context("classify source")?;
    let bytes = runtime::block_on(async {
        let c = IndexClient::new()?;
        c.fetch_profile(src.clone()).await
    })?;
    if !no_validate {
        yoke_config::parse(&bytes).context("validate fetched profile")?;
    }
    let dest_name = as_name.map_or_else(|| derive_name(&src), str::to_string);
    if dry_run {
        return out.emit(
            &serde_json::json!({
                "action": "would-install",
                "dest": &dest_name,
                "bytes": bytes.len(),
            }),
            |w| {
                writeln!(
                    w,
                    "dry-run: would write {} ({} bytes)",
                    dest_name,
                    bytes.len()
                )
            },
        );
    }
    let pn = ProfileName::new(&dest_name)?;
    provider.write_profile(&pn, &bytes)?;
    out.emit(
        &serde_json::json!({
            "action": "installed",
            "dest": &dest_name,
            "bytes": bytes.len(),
            "validated": !no_validate,
        }),
        |w| writeln!(w, "installed {} ({} bytes)", dest_name, bytes.len()),
    )
}

fn derive_name(src: &ProfileSource) -> String {
    match src {
        ProfileSource::LocalPath(p) => p
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("profile")
            .to_string(),
        ProfileSource::Url(u) => u
            .path_segments()
            .and_then(|mut s| s.next_back())
            .map(|s| s.strip_suffix(".csv").unwrap_or(s))
            .filter(|s| !s.is_empty())
            .unwrap_or("profile")
            .to_string(),
        ProfileSource::IndexEntry(n) => n.replace(' ', "_").to_lowercase(),
    }
}
