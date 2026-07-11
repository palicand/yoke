use std::path::PathBuf;
use std::sync::Arc;

#[cfg(any(target_os = "macos", target_os = "windows"))]
use anyhow::Context;
use anyhow::Result;
use yoke_volume::VolumeProvider;
use yoke_volume::fs_backend::FsBackend;

pub fn open(fake_volume: Option<PathBuf>) -> Result<Arc<dyn VolumeProvider>> {
    if let Some(path) = fake_volume {
        return Ok(Arc::new(FsBackend::new(path)));
    }
    #[cfg(target_os = "macos")]
    {
        let p = yoke_volume_macos::MacOsVolumeProvider::new()
            .context("constructing macOS volume provider")?;
        Ok(Arc::new(p))
    }
    #[cfg(target_os = "windows")]
    {
        let p = yoke_volume_windows::WindowsVolumeProvider::new()
            .context("constructing Windows volume provider")?;
        Ok(Arc::new(p))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        anyhow::bail!("no platform volume backend available on this OS; use --fake-volume <path>")
    }
}
