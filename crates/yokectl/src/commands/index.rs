use std::io::Write;

use anyhow::Result;
use yoke_index::{COMMUNITY_INDEX_HTML_URL, IndexClient};

use crate::commands::browser::open_or_emit;
use crate::output::Output;
use crate::runtime;

pub fn run_list(out: &Output, refresh: bool) -> Result<()> {
    let entries = runtime::block_on(async {
        let c = IndexClient::new()?;
        c.list(refresh).await
    })?;
    out.emit(
        &serde_json::json!({
            "entries": entries.iter().map(|e| serde_json::json!({
                "name": e.name,
                "csv_url": e.csv_url.as_str(),
                "fields": &e.fields,
            })).collect::<Vec<_>>(),
        }),
        |w| {
            for e in &entries {
                writeln!(w, "{:40}  {}", e.name, e.csv_url)?;
            }
            Ok(())
        },
    )
}

pub fn run_search(out: &Output, query: &str) -> Result<()> {
    let entries = runtime::block_on(async {
        let c = IndexClient::new()?;
        c.list(false).await
    })?;
    let needle = query.to_ascii_lowercase();
    let filtered: Vec<_> = entries
        .iter()
        .filter(|e| e.name.to_ascii_lowercase().contains(&needle))
        .collect();
    out.emit(
        &serde_json::json!({
            "entries": filtered.iter().map(|e| serde_json::json!({
                "name": e.name,
                "csv_url": e.csv_url.as_str(),
            })).collect::<Vec<_>>(),
        }),
        |w| {
            for e in &filtered {
                writeln!(w, "{:40}  {}", e.name, e.csv_url)?;
            }
            Ok(())
        },
    )
}

pub fn run_show(out: &Output, name: &str) -> Result<()> {
    let entry = runtime::block_on(async {
        let c = IndexClient::new()?;
        c.resolve(name).await
    })?;
    out.emit(
        &serde_json::json!({
            "name": entry.name,
            "csv_url": entry.csv_url.as_str(),
            "fields": entry.fields,
        }),
        |w| {
            writeln!(w, "name: {}", entry.name)?;
            writeln!(w, "csv_url: {}", entry.csv_url)?;
            for (k, v) in &entry.fields {
                writeln!(w, "{k}: {v}")?;
            }
            Ok(())
        },
    )
}

pub fn run_update(out: &Output) -> Result<()> {
    let entries = runtime::block_on(async {
        let c = IndexClient::new()?;
        c.list(true).await
    })?;
    out.emit(&serde_json::json!({"refreshed": entries.len()}), |w| {
        writeln!(w, "refreshed: {} entries", entries.len())
    })
}

pub fn run_browse(out: &Output) -> Result<()> {
    open_or_emit(
        out,
        &serde_json::json!({ "url": COMMUNITY_INDEX_HTML_URL }),
        COMMUNITY_INDEX_HTML_URL,
    )
}
