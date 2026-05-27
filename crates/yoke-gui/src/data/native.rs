use std::path::Path;
use std::sync::Arc;

use tokio::runtime::Runtime;
use tokio::sync::watch;
use yoke_config::model::Profile;
use yoke_index::{IndexClient, IndexEntry, ProfileSource as IndexSource};
use yoke_volume::state::MountState;
use yoke_volume::{ProfileName, VolumeProvider};

use crate::data::{DataError, DataSource, ProfileEntryView};

pub struct NativeDataSource {
    volume: Arc<dyn VolumeProvider>,
    runtime: Runtime,
    /// `None` if the community index could not be initialized (e.g. no cache
    /// dir). Volume + file open still work; community calls report the error.
    index: Option<IndexClient>,
}

impl NativeDataSource {
    /// Construct the production data source: real macOS volume provider (or a
    /// fallback) plus the community index client.
    ///
    /// # Errors
    /// Returns an error only if the tokio runtime cannot be built; a missing
    /// community cache dir is tolerated (community calls then fail at use).
    pub fn new(volume: Arc<dyn VolumeProvider>) -> Result<Self, DataError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| DataError::Volume(e.to_string()))?;
        let index = IndexClient::new().ok();
        if index.is_none() {
            tracing::warn!("community index unavailable (no cache dir); community list disabled");
        }
        Ok(Self { volume, runtime, index })
    }

    /// Test constructor: no community index, current-thread runtime is fine.
    #[doc(hidden)]
    #[must_use]
    pub fn for_test(volume: Arc<dyn VolumeProvider>) -> Self {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("current-thread runtime");
        Self { volume, runtime, index: None }
    }

    /// Watch handle for the worker to wire `VolumeChanged` -> `request_repaint`.
    #[must_use]
    pub fn subscribe_state(&self) -> watch::Receiver<MountState> {
        self.volume.subscribe_state()
    }
}

impl DataSource for NativeDataSource {
    fn volume_state(&self) -> MountState {
        self.volume.current_state()
    }

    fn list_device_profiles(&self) -> Result<Vec<ProfileEntryView>, DataError> {
        let entries = self.volume.list_profiles().map_err(map_volume_err)?;
        Ok(entries
            .into_iter()
            .map(|e| ProfileEntryView { label: e.name.as_filename().to_owned(), name: e.name })
            .collect())
    }

    fn read_device_profile(&self, name: &ProfileName) -> Result<Profile, DataError> {
        let bytes = self.volume.read_profile(name).map_err(map_volume_err)?;
        parse_bytes(&bytes)
    }

    fn read_file_profile(&self, path: &Path) -> Result<Profile, DataError> {
        let bytes = std::fs::read(path).map_err(|e| DataError::File(e.to_string()))?;
        parse_bytes(&bytes)
    }

    fn list_community(&self) -> Result<Vec<IndexEntry>, DataError> {
        let client = self.index.as_ref().ok_or_else(|| {
            DataError::Community("community index unavailable (no cache directory)".into())
        })?;
        let listing = self
            .runtime
            .block_on(client.list(false))
            .map_err(|e| DataError::Community(e.to_string()))?;
        Ok(listing.entries)
    }

    fn fetch_community(&self, entry: &IndexEntry) -> Result<Profile, DataError> {
        let client = self.index.as_ref().ok_or_else(|| {
            DataError::Community("community index unavailable".into())
        })?;
        let src = IndexSource::Url(entry.csv_url.clone());
        let bytes = self
            .runtime
            .block_on(client.fetch_profile(src))
            .map_err(|e| DataError::Community(e.to_string()))?;
        parse_bytes(&bytes)
    }
}

fn parse_bytes(bytes: &[u8]) -> Result<Profile, DataError> {
    yoke_config::parse(bytes)
        .map(|r| r.model)
        .map_err(|e| DataError::Parse(e.to_string()))
}

fn map_volume_err(e: yoke_volume::VolumeError) -> DataError {
    match e {
        yoke_volume::VolumeError::NotPresent => DataError::NotPresent,
        other => DataError::Volume(other.to_string()),
    }
}
