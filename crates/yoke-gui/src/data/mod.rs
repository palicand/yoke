pub mod mock;
#[cfg(not(target_arch = "wasm32"))]
pub mod native;

use std::path::Path;

use yoke_config::model::Profile;

use crate::state::ProfileSource;

#[cfg(not(target_arch = "wasm32"))]
use {yoke_index::IndexEntry, yoke_volume::ProfileName, yoke_volume::state::MountState};

#[cfg(target_arch = "wasm32")]
use crate::data::mock::{MockCommunityEntry as IndexEntry, MockMountState as MountState};
#[cfg(target_arch = "wasm32")]
type ProfileName = String;

#[derive(Debug, thiserror::Error)]
pub enum DataError {
    #[error("profile parse failed: {0}")]
    Parse(String),
    #[error("volume error: {0}")]
    Volume(String),
    #[error("no QuadStick volume mounted")]
    NotPresent,
    #[error("file read failed: {0}")]
    File(String),
    #[error("community index error: {0}")]
    Community(String),
}

/// In-process data provider. No serde, no IPC: passes domain types directly.
/// Implementors must be egui-free.
pub trait DataSource: Send + Sync + 'static {
    fn volume_state(&self) -> MountState;
    fn list_device_profiles(&self) -> Result<Vec<ProfileEntryView>, DataError>;
    fn read_device_profile(&self, name: &ProfileName) -> Result<Profile, DataError>;
    fn read_file_profile(&self, path: &Path) -> Result<Profile, DataError>;
    fn list_community(&self) -> Result<Vec<IndexEntry>, DataError>;
    fn fetch_community(&self, entry: &IndexEntry) -> Result<Profile, DataError>;
    /// Whether the community index is usable. When false the UI skips
    /// `ListCommunity` and renders a disabled (non-retryable) pane instead of a
    /// failure that re-fails identically on every retry.
    fn is_community_available(&self) -> bool {
        true
    }
}

/// Display projection of a device profile entry (decouples views from the
/// native `yoke_volume::ProfileEntry`, which is not present on wasm).
#[derive(Debug, Clone)]
pub struct ProfileEntryView {
    pub name: ProfileName,
    pub label: String,
}

/// Commands sent from the UI to the worker. Open-style commands carry a
/// monotonic request id (`req`) so the UI can drop stale results: a slow open
/// finishing after a newer one must not clobber the editor.
#[derive(Debug, Clone)]
pub enum AppCommand {
    ListDeviceProfiles,
    OpenDeviceProfile { req: u64, name: ProfileName },
    OpenFileDialog { req: u64 },
    ListCommunity,
    OpenCommunity { req: u64, entry: IndexEntry },
}

/// Events sent from the worker back to the UI.
pub enum DataEvent {
    ProfilesListed(Vec<ProfileEntryView>),
    ProfileOpened {
        req: u64,
        source: ProfileSource,
        profile: Box<Profile>,
    },
    CommunityListed(Vec<IndexEntry>),
    VolumeChanged(MountState),
    /// The user dismissed the native file-open dialog. Carries the request id so
    /// a stale cancellation can't clear a newer open's spinner. Distinct from
    /// `Failed` so a real open error with an empty `Display` is never mistaken
    /// for a cancellation.
    FileDialogCancelled {
        req: u64,
    },
    Failed {
        /// `Some` for open-style failures (reconciled against the latest open);
        /// `None` for list failures, which always apply.
        req: Option<u64>,
        context: FailureContext,
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureContext {
    ListDevice,
    OpenDevice,
    OpenFile,
    ListCommunity,
    OpenCommunity,
}

/// Synchronous command dispatch shared by both targets. The async/dialog-only
/// command `OpenFileDialog` is handled in `worker.rs` (the cfg site) and is not
/// routed here.
#[must_use]
pub fn handle_command(data: &dyn DataSource, cmd: AppCommand) -> DataEvent {
    match cmd {
        AppCommand::ListDeviceProfiles => match data.list_device_profiles() {
            Ok(list) => DataEvent::ProfilesListed(list),
            Err(e) => DataEvent::Failed {
                req: None,
                context: FailureContext::ListDevice,
                message: e.to_string(),
            },
        },
        AppCommand::OpenDeviceProfile { req, name } => match data.read_device_profile(&name) {
            Ok(profile) => DataEvent::ProfileOpened {
                req,
                source: ProfileSource::Device(name),
                profile: Box::new(profile),
            },
            Err(e) => DataEvent::Failed {
                req: Some(req),
                context: FailureContext::OpenDevice,
                message: e.to_string(),
            },
        },
        AppCommand::ListCommunity => match data.list_community() {
            Ok(list) => DataEvent::CommunityListed(list),
            Err(e) => DataEvent::Failed {
                req: None,
                context: FailureContext::ListCommunity,
                message: e.to_string(),
            },
        },
        AppCommand::OpenCommunity { req, entry } => {
            let source = community_source(&entry);
            match data.fetch_community(&entry) {
                Ok(profile) => DataEvent::ProfileOpened {
                    req,
                    source,
                    profile: Box::new(profile),
                },
                Err(e) => DataEvent::Failed {
                    req: Some(req),
                    context: FailureContext::OpenCommunity,
                    message: e.to_string(),
                },
            }
        }
        // The browser build has no native dialog (the button is gated off wasm),
        // but routed inline here it must not surface a developer-facing string as
        // a toast; treat it as a benign cancellation. Native intercepts this in
        // `worker` before it reaches here.
        AppCommand::OpenFileDialog { req } => DataEvent::FileDialogCancelled { req },
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn community_source(entry: &IndexEntry) -> ProfileSource {
    ProfileSource::Community {
        name: entry.name.clone(),
        url: entry.csv_url.clone(),
    }
}

#[cfg(target_arch = "wasm32")]
fn community_source(entry: &IndexEntry) -> ProfileSource {
    ProfileSource::Community {
        name: entry.name.clone(),
        url: entry.url.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::mock::MockDataSource;

    #[test]
    fn list_device_profiles_yields_profiles_listed() {
        let data = MockDataSource::new();
        let event = handle_command(&data, AppCommand::ListDeviceProfiles);
        match event {
            DataEvent::ProfilesListed(list) => assert!(!list.is_empty()),
            _ => panic!("expected ProfilesListed"),
        }
    }

    #[test]
    fn list_community_yields_community_listed() {
        let data = MockDataSource::new();
        let event = handle_command(&data, AppCommand::ListCommunity);
        match event {
            DataEvent::CommunityListed(list) => assert!(!list.is_empty()),
            _ => panic!("expected CommunityListed"),
        }
    }
}
