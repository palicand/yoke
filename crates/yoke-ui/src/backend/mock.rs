//! In-memory [`Backend`] for standalone (`trunk serve`) runs.
//!
//! Backs every call with a single baked-in profile fixture so the UI can be
//! exercised end-to-end without a Tauri host or device.

use std::collections::BTreeMap;
use std::path::PathBuf;

use futures::stream;
use yoke_config::parse;
use yoke_ipc::{BackendError, CommunityEntry, DeviceProfileEntry, Profile, VolumePresence};

use super::{Backend, BackendFuture, VolumeStream};

const FIXTURE_CSV: &str = include_str!("../../fixtures/default.csv");

pub struct MockBackend {
    profile: Profile,
}

impl MockBackend {
    /// Build a `MockBackend` by parsing the baked-in fixture CSV.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError::Parse`] if the fixture fails to parse. In
    /// practice this is a build-time concern — the fixture is part of the
    /// crate and is exercised by tests.
    pub fn new() -> Result<Self, BackendError> {
        let parsed =
            parse(FIXTURE_CSV.as_bytes()).map_err(|e| BackendError::Parse(e.to_string()))?;
        Ok(Self {
            profile: parsed.model,
        })
    }
}

impl Backend for MockBackend {
    fn volume_state(&self) -> BackendFuture<'_, VolumePresence> {
        Box::pin(async {
            Ok(VolumePresence::Present {
                label: "Mock Volume".into(),
                mount_point: PathBuf::from("/mock"),
            })
        })
    }

    fn watch_volume_state(&self) -> VolumeStream {
        Box::pin(stream::iter(vec![VolumePresence::Present {
            label: "Mock Volume".into(),
            mount_point: PathBuf::from("/mock"),
        }]))
    }

    fn list_device_profiles(&self) -> BackendFuture<'_, Vec<DeviceProfileEntry>> {
        Box::pin(async {
            Ok(vec![DeviceProfileEntry {
                name: "default.csv".into(),
                kind: "Profile".into(),
            }])
        })
    }

    fn read_device_profile(&self, _name: String) -> BackendFuture<'_, Profile> {
        let profile = self.profile.clone();
        Box::pin(async move { Ok(profile) })
    }

    fn pick_file_dialog(&self) -> BackendFuture<'_, Option<PathBuf>> {
        Box::pin(async { Ok(None) })
    }

    fn read_file_profile(&self, _path: PathBuf) -> BackendFuture<'_, Profile> {
        let profile = self.profile.clone();
        Box::pin(async move { Ok(profile) })
    }

    fn list_community_profiles(&self) -> BackendFuture<'_, Vec<CommunityEntry>> {
        Box::pin(async {
            let mut fields = BTreeMap::new();
            fields.insert("variant".into(), "FPS".into());
            Ok(vec![CommunityEntry {
                name: "Sample community FPS".into(),
                url: "mock://community/sample.csv".into(),
                fields,
            }])
        })
    }

    fn fetch_community_profile(&self, _url: String) -> BackendFuture<'_, Profile> {
        let profile = self.profile.clone();
        Box::pin(async move { Ok(profile) })
    }
}
