use crate::disk_arbitration as da;
use crate::iokit_usb as usb;
use crate::run_loop::{RunLoopThread, RunLoopWorker};
use core_foundation_sys::base::{CFRelease, kCFAllocatorDefault};
use core_foundation_sys::date::CFAbsoluteTimeGetCurrent;
use core_foundation_sys::dictionary::CFDictionaryGetValue;
use core_foundation_sys::runloop::{
    CFRunLoopAddTimer, CFRunLoopGetCurrent, CFRunLoopRef, CFRunLoopTimerContext,
    CFRunLoopTimerCreate, CFRunLoopTimerInvalidate, CFRunLoopTimerRef,
};
use core_foundation_sys::string::{
    CFStringCreateWithCString, CFStringGetCString, CFStringGetLength, CFStringRef,
    kCFStringEncodingUTF8,
};
use core_foundation_sys::url::{CFURLCopyFileSystemPath, CFURLRef, kCFURLPOSIXPathStyle};
use std::collections::HashSet;
use std::ffi::{CStr, CString, c_void};
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, watch};
use yoke_volume::error::VolumeError;
use yoke_volume::io;
use yoke_volume::profile::{ProfileEntry, ProfileName};
use yoke_volume::provider::VolumeProvider;
use yoke_volume::state::{HORI_PS4_VID_PID, ModeHint, MountEvent, MountState, VidPid};

const EVENT_CHANNEL_CAPACITY: usize = 64;
const USB_POLL_INTERVAL_SECS: f64 = 1.0;

unsafe extern "C" {
    static kCFRunLoopDefaultMode: CFStringRef;
}

pub struct MacOsVolumeProvider {
    inner: Arc<Inner>,
    _thread: RunLoopThread,
}

pub struct Inner {
    pub state: Mutex<MountState>,
    pub state_tx: watch::Sender<MountState>,
    pub event_tx: broadcast::Sender<MountEvent>,
    pub tracked: Mutex<Tracked>,
}

#[derive(Default)]
pub struct Tracked {
    pub quadstick_vid_pids: HashSet<VidPid>,
    pub hori_seen: bool,
    pub mount_point: Option<PathBuf>,
    pub label: Option<String>,
}

impl Tracked {
    fn compute(&self) -> MountState {
        if let Some(vp) = self.quadstick_vid_pids.iter().next().copied() {
            if let (Some(mp), Some(lbl)) = (self.mount_point.as_ref(), self.label.as_ref()) {
                return MountState::Present {
                    mount_point: mp.clone(),
                    vid_pid: vp,
                    label: lbl.clone(),
                };
            }
            return MountState::DeviceVisibleNoVolume {
                vid_pid: vp,
                mode_hint: Some(ModeHint::MassStorageDisabled),
            };
        }
        if self.hori_seen {
            return MountState::DeviceVisibleNoVolume {
                vid_pid: HORI_PS4_VID_PID,
                mode_hint: Some(ModeHint::Ps4OrHori),
            };
        }
        MountState::Absent
    }
}

struct Worker {
    inner: Arc<Inner>,
    session: Option<da::DASessionRef>,
    refcon: Option<*mut c_void>,
    timer: Option<CFRunLoopTimerRef>,
}

// SAFETY: `session`, `refcon`, and `timer` are accessed only on the dedicated
// CFRunLoop thread that owns the Worker. The Worker is moved into that thread
// during `RunLoopThread::spawn` and never read from any other thread.
unsafe impl Send for Worker {}

impl Worker {
    const fn new(inner: Arc<Inner>) -> Self {
        Self {
            inner,
            session: None,
            refcon: None,
            timer: None,
        }
    }
}

fn drain_usb_devices(inner: &Inner) {
    let mut new_quadsticks: HashSet<VidPid> = HashSet::new();
    let mut hori_seen = false;
    unsafe {
        let matching = usb::IOServiceMatching(usb::kIOUSBDeviceClassName.as_ptr().cast());
        if matching.is_null() {
            return;
        }
        let mut iter: usb::io_iterator_t = 0;
        let kr = usb::IOServiceGetMatchingServices(
            usb::kIOMainPortDefault,
            matching,
            ptr::addr_of_mut!(iter),
        );
        if kr != 0 {
            return;
        }
        let mut entry = usb::IOIteratorNext(iter);
        while entry != 0 {
            if let (Some(vid), Some(pid)) = (
                usb::read_u16_property(entry, "idVendor"),
                usb::read_u16_property(entry, "idProduct"),
            ) {
                let vp = VidPid {
                    vendor: vid,
                    product: pid,
                };
                match usb::classify(vp) {
                    usb::DeviceClass::QuadStick(vp) => {
                        new_quadsticks.insert(vp);
                    }
                    usb::DeviceClass::HoriPs4 => {
                        hori_seen = true;
                    }
                    usb::DeviceClass::Other => {}
                }
            }
            usb::IOObjectRelease(entry);
            entry = usb::IOIteratorNext(iter);
        }
        usb::IOObjectRelease(iter);
    }
    let mut tracked = inner.tracked.lock().unwrap();
    tracked.quadstick_vid_pids = new_quadsticks;
    tracked.hori_seen = hori_seen;
}

fn drain_volumes(inner: &Inner) {
    let mut found: Option<(PathBuf, String)> = None;
    let Ok(entries) = std::fs::read_dir(Path::new("/Volumes")) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let name_lossy = name.to_string_lossy();
        if name_lossy.eq_ignore_ascii_case("Quad Stick")
            || name_lossy.eq_ignore_ascii_case("QuadStick")
        {
            found = Some((path, name_lossy.into_owned()));
            break;
        }
    }
    let mut tracked = inner.tracked.lock().unwrap();
    if let Some((mp, lbl)) = found {
        tracked.mount_point = Some(mp);
        tracked.label = Some(lbl);
    } else {
        tracked.mount_point = None;
        tracked.label = None;
    }
}

fn publish(inner: &Inner) {
    let new_state = inner.tracked.lock().unwrap().compute();
    let old = std::mem::replace(&mut *inner.state.lock().unwrap(), new_state.clone());
    let _ = inner.state_tx.send(new_state.clone());
    if let Some(evt) = state_transition_event(&old, &new_state) {
        let _ = inner.event_tx.send(evt);
    }
}

fn state_transition_event(old: &MountState, new_state: &MountState) -> Option<MountEvent> {
    match (old, new_state) {
        (
            MountState::Absent | MountState::DeviceVisibleNoVolume { .. },
            MountState::Present {
                mount_point,
                vid_pid,
                label,
            },
        ) => Some(MountEvent::VolumeMounted {
            mount_point: mount_point.clone(),
            vid_pid: *vid_pid,
            label: label.clone(),
        }),
        (
            MountState::Present { .. },
            MountState::Absent | MountState::DeviceVisibleNoVolume { .. },
        ) => Some(MountEvent::VolumeUnmounted),
        (MountState::Absent, MountState::DeviceVisibleNoVolume { vid_pid, .. }) => {
            Some(MountEvent::DeviceAppeared { vid_pid: *vid_pid })
        }
        (MountState::DeviceVisibleNoVolume { .. }, MountState::Absent) => {
            Some(MountEvent::DeviceDisappeared)
        }
        _ => None,
    }
}

unsafe extern "C" fn on_disk_appeared(disk: da::DADiskRef, refcon: *mut c_void) {
    // SAFETY: refcon is the raw Arc pointer leaked in Worker::setup; the Worker
    // owns the leaked strong ref and reclaims it only in teardown after the
    // session has been released (which unregisters DA callbacks). So while a
    // callback is running, the Arc strong ref is live.
    let inner = unsafe { &*(refcon.cast::<Inner>()) };
    handle_disk_appeared(inner, disk);
}

unsafe extern "C" fn on_disk_disappeared(disk: da::DADiskRef, refcon: *mut c_void) {
    // SAFETY: same as on_disk_appeared.
    let inner = unsafe { &*(refcon.cast::<Inner>()) };
    handle_disk_disappeared(inner, disk);
}

extern "C" fn poll_usb(_timer: CFRunLoopTimerRef, refcon: *mut c_void) {
    // SAFETY: same as on_disk_appeared; the timer is invalidated in teardown
    // before the leaked Arc is reclaimed.
    let inner = unsafe { &*(refcon.cast::<Inner>()) };
    drain_usb_devices(inner);
    drain_volumes(inner);
    publish(inner);
}

fn handle_disk_appeared(inner: &Inner, disk: da::DADiskRef) {
    let bsd = unsafe { da::DADiskGetBSDName(disk) };
    if bsd.is_null() {
        return;
    }
    // Confirm the appearing disk is on a QuadStick USB ancestor before
    // touching tracked state. Walk: IOMedia -> ancestors -> first IOUSBDevice.
    let Some(media) = (unsafe { usb::iomedia_for_bsd_name(bsd) }) else {
        return;
    };
    let vid_pid = unsafe { usb::find_usb_ancestor_vid_pid(media) };
    unsafe { usb::IOObjectRelease(media) };
    let Some(vp) = vid_pid else {
        return;
    };
    if !matches!(usb::classify(vp), usb::DeviceClass::QuadStick(_)) {
        return;
    }
    let mount_point = volume_path_from_disk(disk);
    let label = mount_point
        .as_ref()
        .and_then(|p| p.file_name().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "QuadStick".into());
    {
        let mut t = inner.tracked.lock().unwrap();
        t.quadstick_vid_pids.insert(vp);
        if let Some(mp) = mount_point {
            t.mount_point = Some(mp);
            t.label = Some(label);
        }
    }
    publish(inner);
}

fn handle_disk_disappeared(inner: &Inner, _disk: da::DADiskRef) {
    {
        let mut t = inner.tracked.lock().unwrap();
        t.mount_point = None;
        t.label = None;
    }
    publish(inner);
}

fn volume_path_from_disk(disk: da::DADiskRef) -> Option<PathBuf> {
    unsafe {
        let desc = da::DADiskCopyDescription(disk);
        if desc.is_null() {
            return None;
        }
        let key_c = CString::new("DAVolumePath").ok()?;
        let key_cf = CFStringCreateWithCString(ptr::null(), key_c.as_ptr(), kCFStringEncodingUTF8);
        if key_cf.is_null() {
            CFRelease(desc.cast());
            return None;
        }
        let url_ptr = CFDictionaryGetValue(desc, key_cf.cast());
        CFRelease(key_cf.cast());
        if url_ptr.is_null() {
            CFRelease(desc.cast());
            return None;
        }
        let cf_path =
            CFURLCopyFileSystemPath(url_ptr.cast::<_>() as CFURLRef, kCFURLPOSIXPathStyle);
        CFRelease(desc.cast());
        if cf_path.is_null() {
            return None;
        }
        // CFStringGetLength returns CFIndex (signed isize); a negative value
        // would indicate API misuse and we treat it as zero.
        let length_raw = CFStringGetLength(cf_path);
        let length = usize::try_from(length_raw).unwrap_or(0);
        // Worst-case UTF-8 expansion is 4 bytes per UTF-16 code unit, plus NUL.
        let buf_size = length.saturating_mul(4).saturating_add(1);
        let mut buf = vec![0u8; buf_size];
        let buf_size_isize = isize::try_from(buf_size).unwrap_or(isize::MAX);
        let ok = CFStringGetCString(
            cf_path,
            buf.as_mut_ptr().cast(),
            buf_size_isize,
            kCFStringEncodingUTF8,
        );
        CFRelease(cf_path.cast());
        if ok == 0 {
            return None;
        }
        let s = CStr::from_ptr(buf.as_ptr().cast())
            .to_string_lossy()
            .into_owned();
        Some(PathBuf::from(s))
    }
}

impl RunLoopWorker for Worker {
    fn setup(&mut self, _run_loop: CFRunLoopRef) {
        unsafe {
            let session = da::DASessionCreate(ptr::null());
            if session.is_null() {
                tracing::warn!("DASessionCreate returned null");
                return;
            }
            let run_loop = CFRunLoopGetCurrent();
            da::DASessionScheduleWithRunLoop(session, run_loop, kCFRunLoopDefaultMode);

            let refcon_arc = Arc::clone(&self.inner);
            let refcon = Arc::into_raw(refcon_arc).cast::<c_void>().cast_mut();

            da::DARegisterDiskAppearedCallback(session, ptr::null(), on_disk_appeared, refcon);
            da::DARegisterDiskDisappearedCallback(
                session,
                ptr::null(),
                on_disk_disappeared,
                refcon,
            );

            let mut ctx = CFRunLoopTimerContext {
                version: 0,
                info: refcon,
                retain: None,
                release: None,
                copyDescription: None,
            };
            let now = CFAbsoluteTimeGetCurrent();
            let timer = CFRunLoopTimerCreate(
                kCFAllocatorDefault,
                now + USB_POLL_INTERVAL_SECS,
                USB_POLL_INTERVAL_SECS,
                0,
                0,
                poll_usb,
                ptr::addr_of_mut!(ctx),
            );
            CFRunLoopAddTimer(run_loop, timer, kCFRunLoopDefaultMode);

            self.session = Some(session);
            self.refcon = Some(refcon);
            self.timer = Some(timer);
        }

        drain_usb_devices(&self.inner);
        drain_volumes(&self.inner);
        publish(&self.inner);
    }

    fn teardown(&mut self) {
        if let Some(timer) = self.timer.take() {
            unsafe {
                CFRunLoopTimerInvalidate(timer);
                CFRelease(timer.cast());
            }
        }
        if let Some(session) = self.session.take() {
            // CFRelease on the session unregisters any DA callbacks bound to it.
            unsafe { CFRelease(session.cast()) };
        }
        if let Some(refcon) = self.refcon.take() {
            // SAFETY: refcon is the leaked Arc strong ref from setup; reclaim
            // it now that no DA callback or timer can fire (the timer has been
            // invalidated and the session has been released above).
            unsafe { drop(Arc::from_raw(refcon.cast::<Inner>())) };
        }
    }
}

impl MacOsVolumeProvider {
    // Result surface kept for BackendInit errors; DA session failure is a
    // warning-only path today but callers should not assume this always succeeds.
    #[allow(clippy::unnecessary_wraps)]
    pub fn new() -> Result<Self, VolumeError> {
        let (state_tx, _) = watch::channel(MountState::Absent);
        let (event_tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        let inner = Arc::new(Inner {
            state: Mutex::new(MountState::Absent),
            state_tx,
            event_tx,
            tracked: Mutex::new(Tracked::default()),
        });
        let worker = Worker::new(Arc::clone(&inner));
        let thread = RunLoopThread::spawn(worker);
        Ok(Self {
            inner,
            _thread: thread,
        })
    }

    fn require_present<T>(
        &self,
        f: impl FnOnce(&Path) -> Result<T, VolumeError>,
    ) -> Result<T, VolumeError> {
        let state = self.inner.state.lock().unwrap().clone();
        match state {
            MountState::Absent => Err(VolumeError::NotPresent),
            MountState::DeviceVisibleNoVolume { mode_hint, .. } => {
                Err(VolumeError::VolumeHidden { hint: mode_hint })
            }
            MountState::Present { mount_point, .. } => f(&mount_point),
        }
    }
}

impl VolumeProvider for MacOsVolumeProvider {
    fn current_state(&self) -> MountState {
        self.inner.state.lock().unwrap().clone()
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

    #[test]
    fn new_then_drop_joins_cleanly() {
        let provider = MacOsVolumeProvider::new().unwrap();
        let state = provider.current_state();
        eprintln!("current_state after init: {state:?}");
        drop(provider);
    }

    #[test]
    fn io_returns_not_present_or_proceeds() {
        let provider = MacOsVolumeProvider::new().unwrap();
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
