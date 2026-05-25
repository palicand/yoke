use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::watch;
use tracing::warn;
use yoke_ipc::VolumePresence;
use yoke_volume::VolumeProvider;
use yoke_volume::state::MountState;

use crate::commands::volume::presence_from_state;

pub const VOLUME_EVENT: &str = "yoke://volume-state";

pub fn spawn(app: AppHandle, provider: &Arc<dyn VolumeProvider>) {
    let mut rx: watch::Receiver<MountState> = provider.subscribe_state();
    tokio::spawn(async move {
        // Best-effort initial emit for frontends that mount listeners early; the
        // frontend should still invoke `volume_state` on mount for a guaranteed
        // snapshot. `borrow_and_update` clears the unseen-marker so the first
        // `changed().await` blocks until a real state transition.
        let initial = presence_from_state(&rx.borrow_and_update());
        emit(&app, &initial);

        while rx.changed().await.is_ok() {
            let next = presence_from_state(&rx.borrow_and_update());
            emit(&app, &next);
        }
    });
}

fn emit(app: &AppHandle, payload: &VolumePresence) {
    if let Err(e) = app.emit(VOLUME_EVENT, payload) {
        warn!(error = ?e, "failed to emit volume event");
    }
}
