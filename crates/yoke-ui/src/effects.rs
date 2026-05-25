//! Boot-time async effects that drive [`AppState`] signals from the backend.
//!
//! Effects are spawned once from `App` at startup. They run for the lifetime
//! of the document: [`spawn_volume_subscription`] keeps `state.volume` and
//! `state.device_profiles` in sync with the backend's volume-state stream;
//! [`spawn_community_fetch`] is a one-shot loader for the community catalog.

use futures::StreamExt;
use leptos::prelude::*;
use yoke_ipc::VolumePresence;

use crate::state::AppState;

/// Subscribes to the backend volume-state stream and mirrors changes into
/// `state.volume`, refreshing `state.device_profiles` whenever a volume is
/// present and clearing the list otherwise.
pub fn spawn_volume_subscription(state: &AppState) {
    let backend = state.backend.clone();
    let volume = state.volume;
    let device_profiles = state.device_profiles;

    leptos::task::spawn_local(async move {
        let mut stream = backend.watch_volume_state();
        while let Some(p) = stream.next().await {
            volume.set(p.clone());
            if matches!(p, VolumePresence::Present { .. }) {
                if let Ok(entries) = backend.list_device_profiles().await {
                    device_profiles.set(entries);
                }
            } else {
                device_profiles.set(Vec::new());
            }
        }
    });
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
