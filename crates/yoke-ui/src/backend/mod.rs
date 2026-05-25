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

pub mod mock;

use std::path::PathBuf;
use std::pin::Pin;

use futures::Stream;
use yoke_ipc::{BackendError, CommunityEntry, DeviceProfileEntry, Profile, VolumePresence};

pub type BackendResult<T> = Result<T, BackendError>;
pub type VolumeStream = Pin<Box<dyn Stream<Item = VolumePresence> + Send>>;
pub type BackendFuture<'a, T> = Pin<Box<dyn Future<Output = BackendResult<T>> + Send + 'a>>;

pub trait Backend: Send + Sync {
    fn volume_state(&self) -> BackendFuture<'_, VolumePresence>;

    fn watch_volume_state(&self) -> VolumeStream;

    fn list_device_profiles(&self) -> BackendFuture<'_, Vec<DeviceProfileEntry>>;

    fn read_device_profile(&self, name: String) -> BackendFuture<'_, Profile>;

    fn pick_file_dialog(&self) -> BackendFuture<'_, Option<PathBuf>>;

    fn read_file_profile(&self, path: PathBuf) -> BackendFuture<'_, Profile>;

    fn list_community_profiles(&self) -> BackendFuture<'_, Vec<CommunityEntry>>;

    fn fetch_community_profile(&self, url: String) -> BackendFuture<'_, Profile>;
}
