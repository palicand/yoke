use std::path::Path;
use std::sync::Arc;

use tokio::runtime::Runtime;
use tokio::sync::watch;
use yoke_config::ParseResult;
use yoke_index::{IndexClient, IndexEntry, ProfileSource as IndexSource};
use yoke_volume::state::MountState;
use yoke_volume::{ProfileKind, ProfileName, VolumeProvider};

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
        Ok(Self {
            volume,
            runtime,
            index,
        })
    }

    /// Test constructor: no community index, current-thread runtime is fine.
    #[doc(hidden)]
    #[must_use]
    pub fn for_test(volume: Arc<dyn VolumeProvider>) -> Self {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("current-thread runtime");
        Self {
            volume,
            runtime,
            index: None,
        }
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

    fn is_community_available(&self) -> bool {
        self.index.is_some()
    }

    fn list_device_profiles(&self) -> Result<Vec<ProfileEntryView>, DataError> {
        let entries = self.volume.list_profiles().map_err(map_volume_err)?;
        Ok(entries
            .into_iter()
            // prefs.csv holds device settings, not a binding profile; it gets its
            // own editor later and must not appear in the profile library.
            .filter(|e| e.kind != ProfileKind::Prefs)
            .map(|e| {
                let label = e.name.as_filename().to_owned();
                match self.read_device_profile(&e.name) {
                    Ok(parsed) => ProfileEntryView::from_profile(e.name, label, &parsed.model),
                    Err(err) => {
                        tracing::warn!(profile = %label, %err, "profile unreadable; listing without metadata");
                        ProfileEntryView::bare(e.name, label)
                    }
                }
            })
            .collect())
    }

    fn read_device_profile(&self, name: &ProfileName) -> Result<ParseResult, DataError> {
        let bytes = self.volume.read_profile(name).map_err(map_volume_err)?;
        parse_bytes(&bytes)
    }

    fn read_file_profile(&self, path: &Path) -> Result<ParseResult, DataError> {
        let bytes = std::fs::read(path).map_err(|e| DataError::File(e.to_string()))?;
        parse_bytes(&bytes)
    }

    fn write_file_profile(&self, path: &Path, bytes: &[u8]) -> Result<(), DataError> {
        std::fs::write(path, bytes).map_err(|e| DataError::File(e.to_string()))
    }

    fn write_device_profile(&self, name: &ProfileName, bytes: &[u8]) -> Result<(), DataError> {
        self.volume
            .write_profile(name, bytes)
            .map_err(map_volume_err)
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

    fn fetch_community(&self, entry: &IndexEntry) -> Result<ParseResult, DataError> {
        let client = self
            .index
            .as_ref()
            .ok_or_else(|| DataError::Community("community index unavailable".into()))?;
        let src = IndexSource::Url(entry.csv_url.clone());
        let bytes = self
            .runtime
            .block_on(client.fetch_profile(src))
            .map_err(|e| DataError::Community(e.to_string()))?;
        parse_bytes(&bytes)
    }
}

fn parse_bytes(bytes: &[u8]) -> Result<ParseResult, DataError> {
    yoke_config::parse(bytes).map_err(|e| DataError::Parse(e.to_string()))
}

fn map_volume_err(e: yoke_volume::VolumeError) -> DataError {
    match e {
        yoke_volume::VolumeError::NotPresent => DataError::NotPresent,
        other => DataError::Volume(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yoke_volume::FsBackend;

    // Minimal single-sub-profile CSV the parser accepts; content is irrelevant
    // to the filter, which runs before any read.
    const MIN_CSV: &[u8] = b"QuadStick Configuration,Version 1.4,,T\r\n\
Profile Name,,Mouse,\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mouse_left,normal,left,\r\n\
\r\n";

    #[test]
    fn list_device_profiles_excludes_prefs() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("default.csv"), MIN_CSV).unwrap();
        std::fs::write(dir.path().join("halo.csv"), MIN_CSV).unwrap();
        std::fs::write(dir.path().join("prefs.csv"), MIN_CSV).unwrap();

        let backend = Arc::new(FsBackend::new(dir.path().to_path_buf()));
        let data = NativeDataSource::for_test(backend);
        let labels: Vec<String> = data
            .list_device_profiles()
            .unwrap()
            .into_iter()
            .map(|e| e.label)
            .collect();

        assert!(
            labels.iter().any(|l| l == "default.csv"),
            "default.csv must be listed: {labels:?}"
        );
        assert!(
            labels.iter().any(|l| l == "halo.csv"),
            "game profiles must be listed: {labels:?}"
        );
        assert!(
            !labels.iter().any(|l| l.eq_ignore_ascii_case("prefs.csv")),
            "prefs.csv is device settings, not a profile, and must be filtered: {labels:?}"
        );
    }
}
