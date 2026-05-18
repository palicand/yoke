use std::io::Write;
use std::sync::Arc;

use anyhow::Result;
use yoke_volume::VolumeProvider;

use crate::output::{Output, OutputFormat};
use crate::runtime;

pub fn run(provider: &Arc<dyn VolumeProvider>, out: &Output, _include_poll: bool) -> Result<()> {
    // include_poll is reserved for a future low-bandwidth fallback; the current
    // backends already push events, so we ignore it.
    if out.format == OutputFormat::Json {
        runtime::block_on(watch_json(provider.clone(), std::io::stdout()))
    } else {
        runtime::block_on(watch_human(provider.clone()))
    }
}

async fn watch_human(provider: Arc<dyn VolumeProvider>) -> Result<()> {
    let mut events = provider.subscribe_events();
    let mut state = provider.subscribe_state();
    loop {
        tokio::select! {
            evt = events.recv() => {
                if let Ok(e) = evt {
                    println!("[{}] {:?}", now_iso(), e);
                }
            }
            res = state.changed() => {
                if res.is_err() { break; }
                println!("[{}] state: {:?}", now_iso(), *state.borrow());
            }
        }
    }
    Ok(())
}

async fn watch_json<W: Write + Send>(provider: Arc<dyn VolumeProvider>, mut w: W) -> Result<()> {
    let mut events = provider.subscribe_events();
    let mut state = provider.subscribe_state();
    loop {
        tokio::select! {
            evt = events.recv() => {
                if let Ok(e) = evt {
                    let line = serde_json::json!({
                        "timestamp": now_iso(),
                        "kind": "mount-event",
                        "event": format!("{e:?}"),
                    });
                    writeln!(w, "{line}")?;
                }
            }
            res = state.changed() => {
                if res.is_err() { break; }
                let line = serde_json::json!({
                    "timestamp": now_iso(),
                    "kind": "mount-state",
                    "state": format!("{:?}", *state.borrow()),
                });
                writeln!(w, "{line}")?;
            }
        }
    }
    Ok(())
}

// Placeholder RFC-3339-ish timestamp using SystemTime. Pulling chrono in just
// for formatting wasn't worth a dep; promote when a consumer needs real dates.
fn now_iso() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:03}Z", now.as_secs(), now.subsec_millis())
}
