use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use yoke_volume::VolumeProvider;
use yoke_volume::fs_backend::FsBackend;

pub fn open(fake_volume: Option<PathBuf>) -> Result<Arc<dyn VolumeProvider>> {
    if let Some(path) = fake_volume {
        return Ok(Arc::new(FsBackend::new(path)));
    }
    #[cfg(target_os = "macos")]
    {
        use anyhow::Context;

        let p = yoke_volume_macos::MacOsVolumeProvider::new()
            .context("constructing macOS volume provider")?;
        Ok(Arc::new(p))
    }
    #[cfg(not(target_os = "macos"))]
    {
        anyhow::bail!("no platform volume backend available on this OS; use --fake-volume <path>")
    }
}
