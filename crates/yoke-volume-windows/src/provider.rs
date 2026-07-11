use crate::device_notify::DeviceNotifications;
use crate::message_window::{MessageWindowThread, MessageWorker};
use crate::tracked::Tracked;
use crate::usb_enum;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, watch};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    DBT_DEVICEARRIVAL, DBT_DEVICEREMOVECOMPLETE, KillTimer, SetTimer, WM_DEVICECHANGE, WM_TIMER,
};
use yoke_volume::classify::{DeviceClass, DeviceClassifier};
use yoke_volume::error::VolumeError;
use yoke_volume::io;
use yoke_volume::profile::{ProfileEntry, ProfileName};
use yoke_volume::provider::{VolumeProvider, require_present_at};
use yoke_volume::state::{MountEvent, MountState, state_transition_events};

const EVENT_CHANNEL_CAPACITY: usize = 64;
// Device-change notifications don't cover drive-letter assignment or a volume
// becoming readable after its interface arrives, so a periodic rescan self-
// heals those transitions, mirroring the macOS 1 s CFRunLoop poll.
const POLL_INTERVAL_MS: u32 = 1000;
const POLL_TIMER_ID: usize = 1;

pub struct WindowsVolumeProvider {
    inner: Arc<Inner>,
    _thread: MessageWindowThread,
}

struct Inner {
    state_tx: watch::Sender<MountState>,
    event_tx: broadcast::Sender<MountEvent>,
    tracked: Mutex<Tracked>,
    classifier: DeviceClassifier,
}

struct Worker {
    inner: Arc<Inner>,
    notifications: Option<DeviceNotifications>,
    hwnd: Option<HWND>,
}

// SAFETY: `notifications` holds raw HDEVNOTIFY handles that are registered,
// used, and unregistered only on the message-window thread the Worker moves
// into; no other thread ever touches them.
#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl Send for Worker {}

impl MessageWorker for Worker {
    fn setup(&mut self, hwnd: HWND) -> Result<(), VolumeError> {
        self.notifications = Some(DeviceNotifications::register(hwnd)?);
        self.hwnd = Some(hwnd);
        // SAFETY: hwnd is the live message-only window owned by this worker's
        // thread; the timer is killed in `teardown` before the window dies.
        if unsafe { SetTimer(Some(hwnd), POLL_TIMER_ID, POLL_INTERVAL_MS, None) } == 0 {
            tracing::warn!("SetTimer failed; falling back to event-only rescan");
        }
        rescan(&self.inner);
        publish(&self.inner);
        Ok(())
    }

    fn handle_message(&mut self, msg: u32, wparam: WPARAM, _lparam: LPARAM) {
        match msg {
            // Full rescan on any arrival/removal: Tracked is recomputed from a
            // fresh snapshot so a missed or reordered event self-heals on the
            // next one (same philosophy as the macOS 1 s poll).
            WM_DEVICECHANGE => {
                let event = u32::try_from(wparam.0).unwrap_or(0);
                if event == DBT_DEVICEARRIVAL || event == DBT_DEVICEREMOVECOMPLETE {
                    rescan(&self.inner);
                    publish(&self.inner);
                }
            }
            // The periodic poll catches transitions that fire no notification
            // (drive-letter assignment) and lets a transient enumeration
            // failure recover on the next tick instead of stranding.
            WM_TIMER => {
                rescan(&self.inner);
                publish(&self.inner);
            }
            _ => {}
        }
    }

    fn teardown(&mut self) {
        if let Some(hwnd) = self.hwnd.take() {
            // SAFETY: hwnd + timer id are the ones registered in setup on this
            // same thread; the window is still alive during teardown.
            unsafe {
                let _ = KillTimer(Some(hwnd), POLL_TIMER_ID);
            }
        }
        self.notifications = None;
    }
}

// Gating Present on an enumerable filesystem keeps a mid-mount volume out of
// Present, mirroring the macOS EACCES window handling.
fn mount_point_is_ready(path: &Path) -> bool {
    std::fs::read_dir(path).is_ok()
}

fn rescan(inner: &Inner) {
    // A transient CfgMgr32 failure must not be read as "device gone": keep
    // the last good state until the next successful rescan, mirroring the
    // macOS /Volumes read-error handling.
    let (Some(devices), Some(volumes)) =
        (usb_enum::list_usb_devices(), usb_enum::list_usb_volumes())
    else {
        tracing::warn!("usb/volume enumeration failed; keeping last known state");
        return;
    };

    let last_location = inner.tracked.lock().unwrap().quadstick_location.clone();
    let mut new_quadsticks: HashSet<_> = HashSet::new();
    let mut new_location: Option<String> = None;
    let mut hori_seen = false;
    let mut emulation_vp = None;
    for dev in &devices {
        match inner.classifier.classify(dev.vid_pid) {
            DeviceClass::QuadStick(vp) => {
                new_quadsticks.insert(vp);
                if dev.location.is_some() {
                    new_location.clone_from(&dev.location);
                }
            }
            DeviceClass::HoriPs4 => hori_seen = true,
            DeviceClass::Other => {
                // Recognize any device at the port where we last saw a
                // confirmed QuadStick as the same device in an emulation
                // persona we don't have explicitly listed.
                if let (Some(loc), Some(stored)) = (dev.location.as_ref(), last_location.as_ref())
                    && loc == stored
                {
                    emulation_vp = Some(dev.vid_pid);
                }
            }
        }
    }

    let mut volume_devnode_seen = false;
    let mut mount: Option<(PathBuf, String)> = None;
    for vol in &volumes {
        let is_quadstick = matches!(
            inner.classifier.classify(vol.vid_pid),
            DeviceClass::QuadStick(_)
        );
        // A QuadStick in an unlisted emulation persona (recognized by port
        // location, not VID:PID) can still expose its FAT volume; capture that
        // mount so it reads Present, matching the macOS label-based scan.
        let is_emulation = emulation_vp == Some(vol.vid_pid);
        if !is_quadstick && !is_emulation {
            continue;
        }
        if is_quadstick {
            volume_devnode_seen = true;
        }
        if let Some(mp) = &vol.mount_point
            && mount_point_is_ready(mp)
        {
            let label = vol.label.clone().unwrap_or_else(|| "QuadStick".into());
            mount = Some((mp.clone(), label));
        }
    }

    let mut t = inner.tracked.lock().unwrap();
    t.quadstick_vid_pids = new_quadsticks;
    t.hori_seen = hori_seen;
    t.emulation_vp = emulation_vp;
    t.volume_devnode_seen = volume_devnode_seen;
    if let Some(loc) = new_location {
        t.quadstick_location = Some(loc);
    }
    if let Some((mp, lbl)) = mount {
        t.mount_point = Some(mp);
        t.label = Some(lbl);
    } else {
        t.mount_point = None;
        t.label = None;
    }
}

fn publish(inner: &Inner) {
    let new_state = inner.tracked.lock().unwrap().compute();
    let mut emitted_events: Vec<MountEvent> = Vec::new();
    inner.state_tx.send_if_modified(|cur| {
        if *cur == new_state {
            return false;
        }
        emitted_events = state_transition_events(cur, &new_state);
        *cur = new_state.clone();
        true
    });
    for evt in emitted_events {
        let _ = inner.event_tx.send(evt);
    }
}

impl WindowsVolumeProvider {
    pub fn new() -> Result<Self, VolumeError> {
        let (state_tx, _) = watch::channel(MountState::Absent);
        let (event_tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        let classifier = DeviceClassifier::from_env()?;
        let inner = Arc::new(Inner {
            state_tx,
            event_tx,
            tracked: Mutex::new(Tracked::default()),
            classifier,
        });
        let worker = Worker {
            inner: Arc::clone(&inner),
            notifications: None,
            hwnd: None,
        };
        let thread = MessageWindowThread::spawn(worker)?;
        Ok(Self {
            inner,
            _thread: thread,
        })
    }

    fn require_present<T>(
        &self,
        f: impl FnOnce(&Path) -> Result<T, VolumeError>,
    ) -> Result<T, VolumeError> {
        let state = self.inner.state_tx.borrow().clone();
        require_present_at(&state, f)
    }
}

impl VolumeProvider for WindowsVolumeProvider {
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

    fn profile_exists(&self, name: &ProfileName) -> Result<bool, VolumeError> {
        self.require_present(|root| Ok(root.join(name.as_filename()).try_exists()?))
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

    #[test]
    fn new_then_drop_joins_cleanly() {
        let provider = WindowsVolumeProvider::new().unwrap();
        let state = provider.current_state();
        eprintln!("current_state after init: {state:?}");
        drop(provider);
    }

    #[test]
    fn io_returns_not_present_or_proceeds() {
        let provider = WindowsVolumeProvider::new().unwrap();
        let pname = ProfileName::new("x").unwrap();
        match provider.read_profile(&pname) {
            Err(
                VolumeError::NotPresent | VolumeError::VolumeHidden { .. } | VolumeError::Io(_),
            )
            | Ok(_) => {}
            other => panic!("unexpected: {other:?}"),
        }
    }
}
