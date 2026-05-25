use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::watch;
use tracing::warn;
use yoke_ipc::VolumePresence;
use yoke_volume::VolumeProvider;
use yoke_volume::state::MountState;

use crate::commands::volume::presence_from_state;

pub const VOLUME_EVENT: &str = "yoke://volume-state";

#[expect(
    clippy::needless_pass_by_value,
    reason = "callers pass the owning Arc to hand the watcher its own subscription; matches the boot-time setup wiring"
)]
pub fn spawn(app: AppHandle, provider: Arc<dyn VolumeProvider>) {
    let mut rx: watch::Receiver<MountState> = provider.subscribe_state();
    tokio::spawn(async move {
        let initial = presence_from_state(&rx.borrow());
        emit(&app, &initial);

        while rx.changed().await.is_ok() {
            let next = presence_from_state(&rx.borrow());
            emit(&app, &next);
        }
    });
}

fn emit(app: &AppHandle, payload: &VolumePresence) {
    if let Err(e) = app.emit(VOLUME_EVENT, payload) {
        warn!(error = ?e, "failed to emit volume event");
    }
}
