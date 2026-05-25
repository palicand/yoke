use std::io::Write;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::broadcast::error::RecvError;
use yoke_volume::VolumeProvider;

use crate::output::{Output, OutputFormat};
use crate::runtime;

pub fn run(provider: &Arc<dyn VolumeProvider>, out: &Output) -> Result<()> {
    if out.format == OutputFormat::Json {
        runtime::block_on(watch_json(provider.clone(), std::io::stdout()))
    } else {
        runtime::block_on(watch_human(provider.clone()))
    }
}

async fn watch_human(provider: Arc<dyn VolumeProvider>) -> Result<()> {
    let mut events = provider.subscribe_events();
    let mut state = provider.subscribe_state();
    println!("[{}] state: {:?}", now_iso(), *state.borrow());
    loop {
        tokio::select! {
            evt = events.recv() => {
                match evt {
                    Ok(e) => println!("[{}] {:?}", now_iso(), e),
                    Err(RecvError::Closed) => break,
                    Err(RecvError::Lagged(n)) => {
                        println!("[{}] lagged {} events", now_iso(), n);
                    }
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

pub async fn watch_json<W: Write + Send>(
    provider: Arc<dyn VolumeProvider>,
    mut w: W,
) -> Result<()> {
    let mut events = provider.subscribe_events();
    let mut state = provider.subscribe_state();
    let initial = serde_json::json!({
        "timestamp": now_iso(),
        "kind": "mount-state",
        "state": format!("{:?}", *state.borrow()),
    });
    writeln!(w, "{initial}")?;
    loop {
        tokio::select! {
            evt = events.recv() => {
                match evt {
                    Ok(e) => {
                        let line = serde_json::json!({
                            "timestamp": now_iso(),
                            "kind": "mount-event",
                            "event": format!("{e:?}"),
                        });
                        writeln!(w, "{line}")?;
                    }
                    Err(RecvError::Closed) => break,
                    Err(RecvError::Lagged(n)) => {
                        let line = serde_json::json!({
                            "timestamp": now_iso(),
                            "kind": "lagged",
                            "missed": n,
                        });
                        writeln!(w, "{line}")?;
                    }
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

fn now_iso() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| String::from("1970-01-01T00:00:00Z"))
}
