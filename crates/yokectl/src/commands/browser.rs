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

pub fn closest<'a>(query: &str, options: &'a [&'a str]) -> Vec<&'a str> {
    let mut scored: Vec<(usize, &str)> = options
        .iter()
        .map(|o| (strsim::levenshtein(query, o), *o))
        .filter(|(d, _)| *d <= 2 || query.len() <= 3)
        .collect();
    scored.sort_by_key(|(d, _)| *d);
    scored.into_iter().take(5).map(|(_, s)| s).collect()
}
