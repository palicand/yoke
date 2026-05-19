use anyhow::{Context, Result};
use serde::Serialize;

use crate::output::Output;

pub fn open_or_emit<S: Serialize>(out: &Output, payload: &S, url: &str) -> Result<()> {
    if out.is_json() {
        return out.emit(payload, |_| Ok(()));
    }
    opener::open_browser(url).with_context(|| format!("failed to launch browser for {url}"))?;
    println!("opened {url}");
    Ok(())
}

pub fn unknown_slug(kind: &str, slug: &str, known: &[&str]) -> anyhow::Error {
    let suggestions = yoke_edit::suggest::suggestions(slug, known.iter().copied());
    let suggestions_str = if suggestions.is_empty() {
        format!("available: {known:?}")
    } else {
        format!("did you mean: {suggestions:?}")
    };
    anyhow::anyhow!(format!("unknown {kind}: {slug:?}; {suggestions_str}"))
}
