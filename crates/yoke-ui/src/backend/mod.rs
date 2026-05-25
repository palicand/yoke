//! Abstraction over the host that owns I/O for the UI.
//!
//! The UI is dual-targeted: it runs both inside the Tauri shell (real device,
//! filesystem, network) and standalone via `trunk serve` (mocked state). This
//! trait is the seam — Leptos components depend only on `Backend`, never on a
//! concrete implementation.
//!
//! Methods return `Pin<Box<dyn Future<...> + '_>>` rather than using
//! `async-trait`, because Leptos contexts often require explicit boxed futures
//! for sized type erasure. Implementations whose state lives behind `Arc` can
//! clone the `Arc` into the future to produce a `'static` future when needed.

use std::path::PathBuf;
use std::pin::Pin;

use futures::Stream;
use yoke_ipc::{BackendError, CommunityEntry, DeviceProfileEntry, Profile, VolumePresence};

pub type BackendResult<T> = Result<T, BackendError>;
pub type VolumeStream = Pin<Box<dyn Stream<Item = VolumePresence>>>;

pub trait Backend {
    fn volume_state(&self) -> Pin<Box<dyn Future<Output = BackendResult<VolumePresence>> + '_>>;

    fn watch_volume_state(&self) -> VolumeStream;

    fn list_device_profiles(
        &self,
    ) -> Pin<Box<dyn Future<Output = BackendResult<Vec<DeviceProfileEntry>>> + '_>>;

    fn read_device_profile(
        &self,
        name: String,
    ) -> Pin<Box<dyn Future<Output = BackendResult<Profile>> + '_>>;

    fn pick_file_dialog(
        &self,
    ) -> Pin<Box<dyn Future<Output = BackendResult<Option<PathBuf>>> + '_>>;

    fn read_file_profile(
        &self,
        path: PathBuf,
    ) -> Pin<Box<dyn Future<Output = BackendResult<Profile>> + '_>>;

    fn list_community_profiles(
        &self,
    ) -> Pin<Box<dyn Future<Output = BackendResult<Vec<CommunityEntry>>> + '_>>;

    fn fetch_community_profile(
        &self,
        url: String,
    ) -> Pin<Box<dyn Future<Output = BackendResult<Profile>> + '_>>;
}
