use crate::error::VolumeError;
use crate::profile::{ProfileEntry, ProfileName};
use crate::state::{MountEvent, MountState};
use std::io;
use std::path::Path;
use tokio::sync::{broadcast, watch};

pub trait VolumeProvider: Send + Sync + 'static {
    fn current_state(&self) -> MountState;
    fn subscribe_state(&self) -> watch::Receiver<MountState>;
    fn subscribe_events(&self) -> broadcast::Receiver<MountEvent>;

    fn list_profiles(&self) -> Result<Vec<ProfileEntry>, VolumeError>;
    fn read_profile(&self, name: &ProfileName) -> Result<Vec<u8>, VolumeError>;
    fn write_profile(&self, name: &ProfileName, bytes: &[u8]) -> Result<(), VolumeError>;
    fn delete_profile(&self, name: &ProfileName) -> Result<(), VolumeError>;
    fn rename_profile(&self, from: &ProfileName, to: &ProfileName) -> Result<(), VolumeError>;

    /// Whether a profile with `name` already exists on the volume.
    ///
    /// The default impl probes via `read_profile`. Backends that can answer
    /// this without reading the file (FAT directory `try_exists`) should
    /// override.
    fn profile_exists(&self, name: &ProfileName) -> Result<bool, VolumeError> {
        match self.read_profile(name) {
            Ok(_) => Ok(true),
            Err(VolumeError::Io(e)) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e),
        }
    }
}

pub fn require_present_at<T>(
    state: &MountState,
    f: impl FnOnce(&Path) -> Result<T, VolumeError>,
) -> Result<T, VolumeError> {
    match state {
        MountState::Absent => Err(VolumeError::NotPresent),
        MountState::DeviceVisibleNoVolume { mode_hint, .. } => {
            Err(VolumeError::VolumeHidden { hint: *mode_hint })
        }
        MountState::Present { mount_point, .. } => f(mount_point),
    }
}
