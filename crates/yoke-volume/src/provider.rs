use crate::error::VolumeError;
use crate::profile::{ProfileEntry, ProfileName};
use crate::state::{MountEvent, MountState};
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
}
