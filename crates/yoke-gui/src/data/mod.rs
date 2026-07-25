pub mod mock;
#[cfg(not(target_arch = "wasm32"))]
pub mod native;

use std::path::{Path, PathBuf};

use yoke_config::ParseResult;

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
    #[error("file error: {0}")]
    File(String),
    #[error("community index error: {0}")]
    Community(String),
}

/// In-process data provider. No serde, no IPC: passes domain types directly.
/// Implementors must be egui-free.
pub trait DataSource: Send + Sync + 'static {
    fn volume_state(&self) -> MountState;
    fn list_device_profiles(&self) -> Result<Vec<ProfileEntryView>, DataError>;
    fn read_device_profile(&self, name: &ProfileName) -> Result<ParseResult, DataError>;
    fn read_file_profile(&self, path: &Path) -> Result<ParseResult, DataError>;
    fn write_file_profile(&self, path: &Path, bytes: &[u8]) -> Result<(), DataError>;
    fn write_device_profile(&self, name: &ProfileName, bytes: &[u8]) -> Result<(), DataError>;
    fn list_community(&self) -> Result<Vec<IndexEntry>, DataError>;
    fn fetch_community(&self, entry: &IndexEntry) -> Result<ParseResult, DataError>;
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
    pub kind: Option<ProfileKind>,
    pub sub_profiles: usize,
}

impl ProfileEntryView {
    #[must_use]
    pub fn from_profile(
        name: ProfileName,
        label: String,
        profile: &yoke_config::model::Profile,
    ) -> Self {
        let mode_names: Vec<String> = profile
            .sub_profiles
            .iter()
            .map(|s| s.header.mode.canonical_csv())
            .collect();
        let kind = kind_from_mode_names(&mode_names);
        Self {
            name,
            label,
            kind,
            sub_profiles: profile.sub_profiles.len(),
        }
    }

    #[must_use]
    pub const fn bare(name: ProfileName, label: String) -> Self {
        Self {
            name,
            label,
            kind: None,
            sub_profiles: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileKind {
    MouseKeys,
    Gamepad,
    Mixed,
}

impl ProfileKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::MouseKeys => "Mouse + Keys",
            Self::Gamepad => "Gamepad",
            Self::Mixed => "Mixed",
        }
    }
}

/// Classify a profile by the input families its sub-profile modes touch.
///
/// Gamepad family = analog sticks / D-Pad / gamepad / joystick; mouse-keys
/// family = mouse / scroll / keyboard / arrow. Both present is Mixed; neither
/// recognizable is None.
#[must_use]
pub(crate) fn kind_from_mode_names<S: AsRef<str>>(modes: &[S]) -> Option<ProfileKind> {
    let mut gamepad = false;
    let mut mousekeys = false;
    for m in modes {
        let m = m.as_ref().to_lowercase();
        // Accumulate both flags independently: a single free-text mode can
        // name both families (e.g. "Mouse Joystick"), which is Mixed.
        if m.contains("analog")
            || m.contains("d-pad")
            || m.contains("dpad")
            || m.contains("gamepad")
            || m.contains("joystick")
        {
            gamepad = true;
        }
        if m.contains("mouse") || m.contains("scroll") || m.contains("key") || m.contains("arrow") {
            mousekeys = true;
        }
    }
    match (gamepad, mousekeys) {
        (true, true) => Some(ProfileKind::Mixed),
        (true, false) => Some(ProfileKind::Gamepad),
        (false, true) => Some(ProfileKind::MouseKeys),
        (false, false) => None,
    }
}

/// Commands sent from the UI to the worker. Open-style commands carry a
/// monotonic request id (`req`) so the UI can drop stale results: a slow open
/// finishing after a newer one must not clobber the editor.
#[derive(Debug, Clone)]
pub enum AppCommand {
    ListDeviceProfiles,
    OpenDeviceProfile {
        req: u64,
        name: ProfileName,
    },
    OpenFileDialog {
        req: u64,
    },
    ListCommunity,
    OpenCommunity {
        req: u64,
        entry: IndexEntry,
    },
    /// Bytes are pre-serialized by the UI (pure and cheap); only the I/O
    /// belongs on the worker.
    SaveFile {
        req: u64,
        path: PathBuf,
        bytes: Vec<u8>,
    },
    SaveDevice {
        req: u64,
        name: ProfileName,
        bytes: Vec<u8>,
    },
    /// Native worker shows the rfd save dialog; wasm falls through to a
    /// benign cancellation like `OpenFileDialog`. `file_name` seeds the
    /// dialog so the default does not point at an unrelated filename.
    SaveAsDialog {
        req: u64,
        bytes: Vec<u8>,
        file_name: String,
    },
}

/// Events sent from the worker back to the UI.
pub enum DataEvent {
    ProfilesListed(Vec<ProfileEntryView>),
    ProfileOpened {
        req: u64,
        source: ProfileSource,
        parsed: Box<ParseResult>,
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
    Saved {
        req: u64,
        label: String,
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
    SaveFile,
    SaveDevice,
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
            Ok(result) => DataEvent::ProfileOpened {
                req,
                source: ProfileSource::Device(name),
                parsed: Box::new(result),
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
                Ok(result) => DataEvent::ProfileOpened {
                    req,
                    source,
                    parsed: Box::new(result),
                },
                Err(e) => DataEvent::Failed {
                    req: Some(req),
                    context: FailureContext::OpenCommunity,
                    message: e.to_string(),
                },
            }
        }
        // Both dialogs are handled before reaching here (native intercepts in
        // `worker`); on wasm they produce a benign cancellation so no
        // developer-facing string surfaces as a toast.
        AppCommand::OpenFileDialog { req } | AppCommand::SaveAsDialog { req, .. } => {
            DataEvent::FileDialogCancelled { req }
        }
        AppCommand::SaveFile { req, path, bytes } => match data.write_file_profile(&path, &bytes) {
            Ok(()) => DataEvent::Saved {
                req,
                label: path.display().to_string(),
            },
            Err(e) => DataEvent::Failed {
                req: Some(req),
                context: FailureContext::SaveFile,
                message: e.to_string(),
            },
        },
        AppCommand::SaveDevice { req, name, bytes } => {
            match data.write_device_profile(&name, &bytes) {
                Ok(()) => DataEvent::Saved {
                    req,
                    label: device_label(&name),
                },
                Err(e) => DataEvent::Failed {
                    req: Some(req),
                    context: FailureContext::SaveDevice,
                    message: e.to_string(),
                },
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn device_label(name: &ProfileName) -> String {
    format!("QuadStick / {}", name.as_filename())
}

#[cfg(target_arch = "wasm32")]
fn device_label(name: &ProfileName) -> String {
    format!("QuadStick / {name}")
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
    fn kind_from_modes_classifies() {
        use ProfileKind::*;
        assert_eq!(
            kind_from_mode_names(&["Left Analog", "D-Pad"]),
            Some(Gamepad)
        );
        assert_eq!(
            kind_from_mode_names(&["Mouse", "Mouse Scroll", "Arrow keys"]),
            Some(MouseKeys)
        );
        assert_eq!(kind_from_mode_names(&["Mouse", "Left Analog"]), Some(Mixed));
        assert_eq!(kind_from_mode_names(&["Mouse Joystick"]), Some(Mixed));
        assert_eq!(kind_from_mode_names::<&str>(&[]), None);
    }

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

    #[test]
    fn save_device_round_trips_saved_event() {
        let data = MockDataSource::new();
        let event = handle_command(
            &data,
            AppCommand::SaveDevice {
                req: 7,
                name: ProfileName::new("default").unwrap(),
                bytes: b"x".to_vec(),
            },
        );
        match event {
            DataEvent::Saved { req, .. } => assert_eq!(req, 7),
            _ => panic!("expected Saved"),
        }
    }
}
