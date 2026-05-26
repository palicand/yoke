//! Boot-time async effects that drive [`AppState`] signals from the backend.
//!
//! Effects are spawned once from `App` at startup. They run for the lifetime
//! of the document: [`spawn_volume_subscription`] keeps `state.volume` and
//! `state.device_profiles` in sync with the backend's volume-state stream;
//! [`spawn_community_fetch`] is a one-shot loader for the community catalog.

use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use leptos::ev;
use leptos::prelude::*;
use yoke_ipc::{DeviceProfileEntry, VolumePresence};

use crate::backend::Backend;
use crate::state::{AppState, CommunityLoad};

/// Number of times to re-list device profiles before accepting an empty/failed
/// result, and the gap between attempts. Five 200 ms tries give the macOS FAT
/// mount ~1 s to become enumerable after the Present event.
const PROFILE_LIST_RETRIES: usize = 5;
const PROFILE_LIST_RETRY_DELAY: Duration = Duration::from_millis(200);

/// Subscribes to the backend volume-state stream and mirrors changes into
/// `state.volume`, refreshing `state.device_profiles` whenever a volume is
/// present and clearing the list otherwise.
pub fn spawn_volume_subscription(state: &AppState) {
    let backend = state.backend.clone();
    let volume = state.volume;
    let device_profiles = state.device_profiles;
    let toast = state.toast;

    leptos::task::spawn_local(async move {
        // The host emits an initial volume event from its `setup` callback,
        // but that fires before the webview attaches a listener — so the
        // first event is lost and subsequent USB polls only publish on
        // change. Pull the current state explicitly so the UI matches
        // reality on first paint regardless of timing.
        if let Ok(initial) = backend.volume_state().await {
            apply(&backend, volume, device_profiles, toast, initial).await;
        }

        let mut stream = backend.watch_volume_state();
        while let Some(p) = stream.next().await {
            apply(&backend, volume, device_profiles, toast, p).await;
        }
    });
}

#[expect(
    clippy::future_not_send,
    reason = "only ever awaited on spawn_local; the UI runtime is single-threaded"
)]
async fn apply(
    backend: &Arc<dyn Backend>,
    volume: RwSignal<VolumePresence>,
    device_profiles: RwSignal<Vec<DeviceProfileEntry>>,
    toast: RwSignal<Option<String>>,
    p: VolumePresence,
) {
    volume.set(p.clone());
    if matches!(p, VolumePresence::Present { .. }) {
        refresh_device_profiles(backend, volume, device_profiles, toast).await;
    } else {
        device_profiles.set(Vec::new());
    }
}

/// Lists device profiles for a freshly-present volume, retrying briefly while
/// the result is empty or errors.
///
/// macOS publishes Present the instant the volume mounts, but the FAT
/// directory can take a moment to become enumerable. The mount state then
/// doesn't change again, so no second volume event arrives — without this
/// retry the list would stay empty until the user reloaded the app.
#[expect(
    clippy::future_not_send,
    reason = "only ever awaited on spawn_local; the UI runtime is single-threaded"
)]
async fn refresh_device_profiles(
    backend: &Arc<dyn Backend>,
    volume: RwSignal<VolumePresence>,
    device_profiles: RwSignal<Vec<DeviceProfileEntry>>,
    toast: RwSignal<Option<String>>,
) {
    for attempt in 0..PROFILE_LIST_RETRIES {
        // A disable mid-retry makes the listing moot; the Absent/hidden event
        // that follows clears the list on its own.
        if !matches!(volume.get_untracked(), VolumePresence::Present { .. }) {
            return;
        }
        let last_attempt = attempt + 1 == PROFILE_LIST_RETRIES;
        match backend.list_device_profiles().await {
            Ok(entries) if !entries.is_empty() => {
                device_profiles.set(entries);
                return;
            }
            Ok(empty) if last_attempt => {
                device_profiles.set(empty);
                return;
            }
            Err(e) if last_attempt => {
                tracing::error!(error = %e, "listing device profiles failed");
                toast.set(Some(format!("Could not read device profiles: {e}")));
                return;
            }
            Ok(_) | Err(_) => {}
        }
        gloo_timers::future::sleep(PROFILE_LIST_RETRY_DELAY).await;
    }
}

/// Suppresses the webview's native right-click menu in release builds.
///
/// Gives the app a native feel (no "Reload" / "Back" entries). Left enabled in
/// debug builds (`trunk serve`, `tauri dev`) so dev tooling stays reachable.
/// The listener intentionally lives for the whole session.
pub fn suppress_native_context_menu() {
    if cfg!(debug_assertions) {
        return;
    }
    drop(window_event_listener(ev::contextmenu, |e| {
        e.prevent_default();
    }));
}

/// Loads the community profile catalog once at boot.
pub fn spawn_community_fetch(state: &AppState) {
    let backend = state.backend.clone();
    let community = state.community;
    leptos::task::spawn_local(async move {
        match backend.list_community_profiles().await {
            Ok(entries) => community.set(CommunityLoad::Loaded(entries)),
            Err(e) => {
                tracing::error!(error = %e, "listing community profiles failed");
                community.set(CommunityLoad::Failed(e.to_string()));
            }
        }
    });
}
