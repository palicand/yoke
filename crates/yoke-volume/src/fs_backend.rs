use crate::error::VolumeError;
use crate::io;
use crate::profile::{ProfileEntry, ProfileName};
use crate::provider::{VolumeProvider, require_present_at};
use crate::state::{MountEvent, MountState, VidPid, state_transition_event};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{broadcast, watch};

const FS_BACKEND_LABEL: &str = "fs-backend";
const FS_BACKEND_VID_PID: VidPid = VidPid {
    vendor: 0,
    product: 0,
};
const EVENT_CHANNEL_CAPACITY: usize = 64;

pub struct FsBackend {
    inner: Arc<FsInner>,
}

struct FsInner {
    root: PathBuf,
    state_tx: watch::Sender<MountState>,
    event_tx: broadcast::Sender<MountEvent>,
}

impl FsBackend {
    pub fn new(root: PathBuf) -> Self {
        let initial = compute_state(&root);
        let (state_tx, _) = watch::channel(initial);
        let (event_tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        let inner = Arc::new(FsInner {
            root,
            state_tx,
            event_tx,
        });
        Self { inner }
    }

    pub fn set_present(&self, present: bool) {
        let new_state = if present {
            compute_state(&self.inner.root)
        } else {
            MountState::Absent
        };
        let mut emitted_event = None;
        self.inner.state_tx.send_if_modified(|cur| {
            if *cur == new_state {
                return false;
            }
            emitted_event = state_transition_event(cur, &new_state);
            *cur = new_state.clone();
            true
        });
        if let Some(evt) = emitted_event {
            let _ = self.inner.event_tx.send(evt);
        }
    }

    fn require_present<T>(
        &self,
        f: impl FnOnce(&Path) -> Result<T, VolumeError>,
    ) -> Result<T, VolumeError> {
        let state = self.inner.state_tx.borrow().clone();
        require_present_at(&state, f)
    }
}

fn compute_state(root: &Path) -> MountState {
    if root.is_dir() {
        MountState::Present {
            mount_point: root.to_path_buf(),
            vid_pid: FS_BACKEND_VID_PID,
            label: FS_BACKEND_LABEL.to_string(),
        }
    } else {
        MountState::Absent
    }
}

impl VolumeProvider for FsBackend {
    fn current_state(&self) -> MountState {
        self.inner.state_tx.borrow().clone()
    }

    fn subscribe_state(&self) -> watch::Receiver<MountState> {
        self.inner.state_tx.subscribe()
    }

    fn subscribe_events(&self) -> broadcast::Receiver<MountEvent> {
        self.inner.event_tx.subscribe()
    }

    fn list_profiles(&self) -> Result<Vec<ProfileEntry>, VolumeError> {
        self.require_present(io::list_profiles)
    }

    fn read_profile(&self, name: &ProfileName) -> Result<Vec<u8>, VolumeError> {
        self.require_present(|root| io::read_profile(root, name))
    }

    fn write_profile(&self, name: &ProfileName, bytes: &[u8]) -> Result<(), VolumeError> {
        self.require_present(|root| io::write_profile(root, name, bytes))
    }

    fn delete_profile(&self, name: &ProfileName) -> Result<(), VolumeError> {
        self.require_present(|root| io::delete_profile(root, name))
    }

    fn rename_profile(&self, from: &ProfileName, to: &ProfileName) -> Result<(), VolumeError> {
        self.require_present(|root| io::rename_profile(root, from, to))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn pname(s: &str) -> ProfileName {
        ProfileName::new(s).unwrap()
    }

    #[test]
    fn new_returns_present_when_dir_exists() {
        let dir = tempdir().unwrap();
        let backend = FsBackend::new(dir.path().to_path_buf());
        assert!(matches!(
            backend.current_state(),
            MountState::Present { .. }
        ));
    }

    #[test]
    fn new_returns_absent_when_dir_missing() {
        let missing = std::env::temp_dir().join("does-not-exist-yoke-test-12345");
        let _ = std::fs::remove_dir_all(&missing);
        let backend = FsBackend::new(missing);
        assert!(matches!(backend.current_state(), MountState::Absent));
    }

    #[test]
    fn set_present_false_flips_to_absent() {
        let dir = tempdir().unwrap();
        let backend = FsBackend::new(dir.path().to_path_buf());
        backend.set_present(false);
        assert!(matches!(backend.current_state(), MountState::Absent));
    }

    #[test]
    fn set_present_true_flips_back_to_present() {
        let dir = tempdir().unwrap();
        let backend = FsBackend::new(dir.path().to_path_buf());
        backend.set_present(false);
        backend.set_present(true);
        assert!(matches!(
            backend.current_state(),
            MountState::Present { .. }
        ));
    }

    #[test]
    fn io_returns_not_present_when_absent() {
        let missing = std::env::temp_dir().join("does-not-exist-yoke-test-67890");
        let _ = std::fs::remove_dir_all(&missing);
        let backend = FsBackend::new(missing);
        assert!(matches!(
            backend.list_profiles(),
            Err(VolumeError::NotPresent)
        ));
        assert!(matches!(
            backend.read_profile(&pname("x")),
            Err(VolumeError::NotPresent)
        ));
        assert!(matches!(
            backend.write_profile(&pname("x"), b"y"),
            Err(VolumeError::NotPresent)
        ));
        assert!(matches!(
            backend.delete_profile(&pname("x")),
            Err(VolumeError::NotPresent)
        ));
        assert!(matches!(
            backend.rename_profile(&pname("x"), &pname("y")),
            Err(VolumeError::NotPresent)
        ));
    }

    #[test]
    fn write_then_read_round_trip() {
        let dir = tempdir().unwrap();
        let backend = FsBackend::new(dir.path().to_path_buf());
        backend.write_profile(&pname("a"), b"hi").unwrap();
        let bytes = backend.read_profile(&pname("a")).unwrap();
        assert_eq!(&bytes, b"hi");
    }

    #[test]
    fn subscribe_state_initial_value_matches_current_state() {
        let dir = tempdir().unwrap();
        let backend = FsBackend::new(dir.path().to_path_buf());
        let rx = backend.subscribe_state();
        assert_eq!(*rx.borrow(), backend.current_state());
    }
}
