#![allow(non_snake_case, dead_code)]

use core_foundation_sys::base::{CFRelease, CFRetain};
use core_foundation_sys::runloop::{
    CFRunLoopGetCurrent, CFRunLoopRef, CFRunLoopRunInMode, CFRunLoopStop,
};
use core_foundation_sys::string::CFStringRef;
use std::ffi::c_int;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::sync_channel;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use yoke_volume::error::VolumeError;

const K_CF_RUN_LOOP_RAN_STOPPED: c_int = 2;

unsafe extern "C" {
    static kCFRunLoopDefaultMode: CFStringRef;
}

pub struct RunLoopThread {
    handle: Option<JoinHandle<()>>,
    run_loop: Mutex<Option<CFRunLoopRef>>,
    stop_flag: Arc<AtomicBool>,
}

pub trait RunLoopWorker: Send + 'static {
    fn setup(&mut self, run_loop: CFRunLoopRef);
    fn teardown(&mut self);
}

// CFRunLoopRef is a raw pointer so Send/Sync are not auto-derived. Both are
// safe here: CFRunLoop is documented thread-safe and CFRunLoopStop is the
// only cross-thread call we make, which is its documented stop primitive.
unsafe impl Send for RunLoopThread {}
unsafe impl Sync for RunLoopThread {}

struct RunLoopHandle(CFRunLoopRef);

unsafe impl Send for RunLoopHandle {}

impl RunLoopThread {
    pub fn spawn<W: RunLoopWorker>(mut worker: W) -> Result<Self, VolumeError> {
        let (tx, rx) = sync_channel::<RunLoopHandle>(0);
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = Arc::clone(&stop_flag);
        let handle = std::thread::Builder::new()
            .name("yoke-volume-da".into())
            .spawn(move || {
                let run_loop: CFRunLoopRef = unsafe { CFRunLoopGetCurrent() };
                // CF Get-rule: CFRunLoopGetCurrent is not +1, retain so the
                // stored handle outlives this thread.
                unsafe { CFRetain(run_loop.cast()) };
                worker.setup(run_loop);
                if tx.send(RunLoopHandle(run_loop)).is_err() {
                    worker.teardown();
                    unsafe { CFRelease(run_loop.cast()) };
                    return;
                }
                drop(tx);
                // Pump in short slices so the stop flag is observed even when
                // no CF sources are registered (CFRunLoopRun would otherwise
                // return immediately and race CFRunLoopStop from Drop).
                loop {
                    let result = unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.1, 0) };
                    if stop_flag_clone.load(Ordering::Acquire) {
                        break;
                    }
                    if result == K_CF_RUN_LOOP_RAN_STOPPED {
                        break;
                    }
                    let _ = result;
                }
                worker.teardown();
            })
            .map_err(|e| VolumeError::BackendInit(format!("spawn DA thread: {e}")))?;
        let run_loop = rx
            .recv()
            .map_err(|_| VolumeError::BackendInit("DA thread exited before seeding".into()))?;
        Ok(Self {
            handle: Some(handle),
            run_loop: Mutex::new(Some(run_loop.0)),
            stop_flag,
        })
    }
}

impl Drop for RunLoopThread {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::Release);
        let run_loop = self.run_loop.lock().unwrap().take();
        if let Some(rl) = run_loop {
            unsafe {
                CFRunLoopStop(rl);
                CFRelease(rl.cast());
            }
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TrivialWorker {
        setup_called: Arc<AtomicBool>,
        teardown_called: Arc<AtomicBool>,
    }

    impl RunLoopWorker for TrivialWorker {
        fn setup(&mut self, _rl: CFRunLoopRef) {
            self.setup_called.store(true, Ordering::SeqCst);
        }
        fn teardown(&mut self) {
            self.teardown_called.store(true, Ordering::SeqCst);
        }
    }

    #[test]
    fn spawn_and_drop_joins_cleanly() {
        let setup = Arc::new(AtomicBool::new(false));
        let teardown = Arc::new(AtomicBool::new(false));
        let worker = TrivialWorker {
            setup_called: setup.clone(),
            teardown_called: teardown.clone(),
        };
        let thread = RunLoopThread::spawn(worker);
        assert!(
            setup.load(Ordering::SeqCst),
            "setup must run before spawn returns"
        );
        drop(thread);
        assert!(
            teardown.load(Ordering::SeqCst),
            "teardown must run after stop"
        );
    }
}
