//! App-level Leptos state shared via context.
//!
//! `AppState` carries the backend handle plus the reactive signals that the UI
//! tree reads and writes: volume presence, device/community profile lists, the
//! currently-open profile, and the latest toast message. Child components grab
//! this via [`use_state`].

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

#[derive(Clone)]
pub struct AppState {
    pub backend: Arc<dyn Backend>,
    pub volume: RwSignal<VolumePresence>,
    pub device_profiles: RwSignal<Vec<DeviceProfileEntry>>,
    pub community_profiles: RwSignal<Vec<CommunityEntry>>,
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
            community_profiles: RwSignal::new(Vec::new()),
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
