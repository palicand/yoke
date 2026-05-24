use std::io::Write;
use std::sync::Arc;

use anyhow::{Context, Result};
use percent_encoding::percent_decode_str;
use yoke_index::ProfileSource;
use yoke_volume::VolumeProvider;
use yoke_volume::profile::{ProfileName, sanitize_for_profile_name};

use crate::error::CliError;
use crate::output::Output;
use crate::runtime;

pub fn run(
    provider: &Arc<dyn VolumeProvider>,
    out: &Output,
    source: &str,
    as_name: Option<&str>,
    dry_run: bool,
    no_validate: bool,
    force: bool,
) -> Result<()> {
    let src = ProfileSource::classify(source).context("classify source")?;
    let bytes = runtime::block_on(yoke_index::fetch_profile_bytes(src.clone()))?;
    if no_validate {
        tracing::warn!("--no-validate: skipping profile schema parse before write");
    } else {
        yoke_config::parse(&bytes).context("validate fetched profile")?;
    }
    let dest_name = as_name.map_or_else(|| derive_name(&src), str::to_string);
    let pn = ProfileName::new(&dest_name)?;
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
    // Auto-derived names can collide silently (`Destiny 2` → `destiny_2`,
    // installed twice from different sources). Require an explicit decision
    // — either `--as` or `--force` — before overwriting. `--as` callers have
    // already named the destination, so we match push/copy's overwrite
    // semantics there.
    if as_name.is_none() && !force && provider.profile_exists(&pn)? {
        return Err(CliError::RequiresForce {
            name: pn.stem().to_string(),
        }
        .into());
    }
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
            .map_or_else(|| "profile".into(), sanitize_for_profile_name),
        ProfileSource::Url(u) => derive_name_from_url(u),
        ProfileSource::IndexEntry(n) => sanitize_for_profile_name(n),
    }
}

fn derive_name_from_url(u: &url::Url) -> String {
    let last = u
        .path_segments()
        .and_then(|mut s| s.next_back())
        .unwrap_or("");
    let decoded = percent_decode_str(last).decode_utf8_lossy();
    let stem = decoded
        .strip_suffix(".csv")
        .or_else(|| decoded.strip_suffix(".CSV"))
        .unwrap_or(&decoded);
    let sanitized = sanitize_for_profile_name(stem);
    // Google Sheets URLs end in /pub, /pubhtml, /edit, /export — these are
    // verbs, not names. Sanitizing them yields a bare verb that collides
    // across every Sheets install. Surface the ambiguity and let the caller
    // disambiguate via `--as`.
    if matches!(sanitized.as_str(), "pub" | "pubhtml" | "edit" | "export") {
        tracing::warn!(
            "URL basename {sanitized:?} is a Sheets verb, not a profile name; pass --as to set one"
        );
        return "profile".into();
    }
    sanitized
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use url::Url;

    #[test]
    fn local_path_derivation_sanitizes() {
        let src = ProfileSource::LocalPath(PathBuf::from("/tmp/Half-Life: Alyx.csv"));
        assert_eq!(derive_name(&src), "half-life_alyx");
    }

    #[test]
    fn index_entry_derivation_sanitizes_spaces() {
        let src = ProfileSource::IndexEntry("Destiny 2".into());
        assert_eq!(derive_name(&src), "destiny_2");
    }

    #[test]
    fn url_derivation_percent_decodes_then_sanitizes() {
        let src = ProfileSource::Url(Url::parse("https://x/Half-Life%3A%20Alyx.csv").unwrap());
        assert_eq!(derive_name(&src), "half-life_alyx");
    }

    #[test]
    fn url_derivation_falls_back_when_basename_is_sheets_verb() {
        for verb in ["pub", "pubhtml", "edit", "export"] {
            let src = ProfileSource::Url(Url::parse(&format!("https://docs.google.com/{verb}")).unwrap());
            assert_eq!(derive_name(&src), "profile", "verb {verb}");
        }
    }

    #[test]
    fn url_derivation_handles_trailing_csv_case_insensitively() {
        let src = ProfileSource::Url(Url::parse("https://x/Foo.CSV").unwrap());
        assert_eq!(derive_name(&src), "foo");
    }
}
