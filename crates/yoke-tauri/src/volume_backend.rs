use std::sync::Arc;
use yoke_volume::VolumeProvider;

#[cfg(all(target_os = "macos", not(feature = "fake-volume")))]
pub fn build_provider() -> anyhow::Result<Arc<dyn VolumeProvider>> {
    let provider = yoke_volume_macos::MacOsVolumeProvider::new()?;
    Ok(Arc::new(provider))
}

#[cfg(any(not(target_os = "macos"), feature = "fake-volume"))]
pub fn build_provider() -> anyhow::Result<Arc<dyn VolumeProvider>> {
    let root = std::env::var("YOKE_FAKE_VOLUME").map_or_else(
        |_| std::env::temp_dir().join("yoke-fake-volume"),
        std::path::PathBuf::from,
    );
    std::fs::create_dir_all(&root)?;
    let fs = yoke_volume::FsBackend::new(root);
    fs.set_present(true);
    Ok(Arc::new(fs))
}
