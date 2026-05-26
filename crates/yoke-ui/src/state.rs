//! App-level Leptos state shared via context.
//!
//! `AppState` carries the backend handle plus the reactive signals that the UI
//! tree reads and writes: volume presence, device profile list and community
//! load state, the currently-open profile, and the latest toast message. Child
//! components grab this via [`use_state`].

use std::path::PathBuf;
use std::sync::Arc;

use leptos::prelude::*;
use yoke_ipc::{CommunityEntry, DeviceProfileEntry, Profile, VolumePresence};

use crate::backend::Backend;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileSource {
    Device(String),
    File(PathBuf),
    Community { name: String, url: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenProfile {
    pub source: ProfileSource,
    pub profile: Profile,
}

/// Load state for the community catalog.
///
/// A plain `Vec` could not distinguish "still fetching" from "fetched but
/// empty" from "fetch failed"; the empty case made the library spin forever
/// when the one-shot boot fetch errored. These variants keep the cases distinct.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum CommunityLoad {
    #[default]
    Loading,
    Loaded(Vec<CommunityEntry>),
    Failed(String),
}

#[derive(Clone)]
pub struct AppState {
    pub backend: Arc<dyn Backend>,
    pub volume: RwSignal<VolumePresence>,
    pub device_profiles: RwSignal<Vec<DeviceProfileEntry>>,
    pub community: RwSignal<CommunityLoad>,
    pub open_profile: RwSignal<Option<OpenProfile>>,
    pub toast: RwSignal<Option<String>>,
}

impl AppState {
    #[must_use]
    pub fn new(backend: Arc<dyn Backend>) -> Self {
        Self {
            backend,
            volume: RwSignal::new(VolumePresence::Absent),
            device_profiles: RwSignal::new(Vec::new()),
            community: RwSignal::new(CommunityLoad::Loading),
            open_profile: RwSignal::new(None),
            toast: RwSignal::new(None),
        }
    }
}

/// Returns the [`AppState`] from the surrounding Leptos context.
///
/// # Panics
///
/// Panics if called outside the `App` tree where `AppState` has been provided.
#[must_use]
pub fn use_state() -> AppState {
    expect_context::<AppState>()
}
