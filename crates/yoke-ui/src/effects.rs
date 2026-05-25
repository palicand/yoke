//! Boot-time async effects that drive [`AppState`] signals from the backend.
//!
//! Effects are spawned once from `App` at startup. They run for the lifetime
//! of the document: [`spawn_volume_subscription`] keeps `state.volume` and
//! `state.device_profiles` in sync with the backend's volume-state stream;
//! [`spawn_community_fetch`] is a one-shot loader for the community catalog.

use std::sync::Arc;

use futures::StreamExt;
use leptos::prelude::*;
use yoke_ipc::{DeviceProfileEntry, VolumePresence};

use crate::backend::Backend;
use crate::state::AppState;

/// Subscribes to the backend volume-state stream and mirrors changes into
/// `state.volume`, refreshing `state.device_profiles` whenever a volume is
/// present and clearing the list otherwise.
pub fn spawn_volume_subscription(state: &AppState) {
    let backend = state.backend.clone();
    let volume = state.volume;
    let device_profiles = state.device_profiles;

    leptos::task::spawn_local(async move {
        // The host emits an initial volume event from its `setup` callback,
        // but that fires before the webview attaches a listener — so the
        // first event is lost and subsequent USB polls only publish on
        // change. Pull the current state explicitly so the UI matches
        // reality on first paint regardless of timing.
        if let Ok(initial) = backend.volume_state().await {
            apply(&backend, volume, device_profiles, initial).await;
        }

        let mut stream = backend.watch_volume_state();
        while let Some(p) = stream.next().await {
            apply(&backend, volume, device_profiles, p).await;
        }
    });
}

async fn apply(
    backend: &Arc<dyn Backend>,
    volume: RwSignal<VolumePresence>,
    device_profiles: RwSignal<Vec<DeviceProfileEntry>>,
    p: VolumePresence,
) {
    volume.set(p.clone());
    if matches!(p, VolumePresence::Present { .. }) {
        if let Ok(entries) = backend.list_device_profiles().await {
            device_profiles.set(entries);
        }
    } else {
        device_profiles.set(Vec::new());
    }
}

/// Loads the community profile catalog once at boot.
pub fn spawn_community_fetch(state: &AppState) {
    let backend = state.backend.clone();
    let community = state.community_profiles;
    leptos::task::spawn_local(async move {
        if let Ok(entries) = backend.list_community_profiles().await {
            community.set(entries);
        }
    });
}
