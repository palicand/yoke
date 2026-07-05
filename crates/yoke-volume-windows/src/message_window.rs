use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Mutex;
use std::sync::mpsc::{SyncSender, sync_channel};
use std::thread::JoinHandle;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, HWND_MESSAGE,
    MSG, PostMessageW, PostQuitMessage, RegisterClassW, TranslateMessage, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_APP, WNDCLASSW,
};
use windows::core::{PCWSTR, w};
use yoke_volume::error::VolumeError;

pub const WM_APP_SHUTDOWN: u32 = WM_APP + 1;
const WINDOW_CLASS: PCWSTR = w!("yoke-volume-windows-msgwin");

pub trait MessageWorker: Send + 'static {
    fn setup(&mut self, hwnd: HWND) -> Result<(), VolumeError>;
    fn handle_message(&mut self, msg: u32, wparam: WPARAM, lparam: LPARAM);
    fn teardown(&mut self);
}

type Handler = Box<dyn FnMut(u32, WPARAM, LPARAM)>;

thread_local! {
    // The window and its wndproc live on exactly one thread; routing through
    // a thread-local avoids GWLP_USERDATA pointer juggling.
    static HANDLER: RefCell<Option<Handler>> = const { RefCell::new(None) };
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_APP_SHUTDOWN {
        unsafe { PostQuitMessage(0) };
        return LRESULT(0);
    }
    HANDLER.with(|h| {
        if let Some(f) = h.borrow_mut().as_mut() {
            f(msg, wparam, lparam);
        }
    });
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

pub struct MessageWindowThread {
    handle: Option<JoinHandle<()>>,
    // Raw HWND value; reconstructed only to post the shutdown message.
    hwnd: Mutex<Option<isize>>,
}

impl MessageWindowThread {
    pub fn spawn<W: MessageWorker>(worker: W) -> Result<Self, VolumeError> {
        let (tx, rx) = sync_channel::<Result<isize, VolumeError>>(0);
        let handle = std::thread::Builder::new()
            .name("yoke-volume-win".into())
            .spawn(move || run_pump(worker, &tx))
            .map_err(|e| VolumeError::BackendInit(format!("spawn message-window thread: {e}")))?;
        match rx.recv() {
            Ok(Ok(hwnd)) => Ok(Self {
                handle: Some(handle),
                hwnd: Mutex::new(Some(hwnd)),
            }),
            Ok(Err(e)) => {
                // Wait for teardown so an immediate retry doesn't race a
                // half-torn-down window class.
                let _ = handle.join();
                Err(e)
            }
            Err(_) => {
                let _ = handle.join();
                Err(VolumeError::BackendInit(
                    "message-window thread exited before seeding".into(),
                ))
            }
        }
    }
}

impl Drop for MessageWindowThread {
    fn drop(&mut self) {
        let hwnd = self.hwnd.lock().unwrap().take();
        if let Some(hwnd) = hwnd {
            let hwnd = HWND(hwnd as *mut core::ffi::c_void);
            // SAFETY: the pump thread keeps the window alive until it sees
            // this message; posting to a destroyed window only fails the call.
            unsafe {
                let _ = PostMessageW(Some(hwnd), WM_APP_SHUTDOWN, WPARAM(0), LPARAM(0));
            }
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn run_pump<W: MessageWorker>(worker: W, tx: &SyncSender<Result<isize, VolumeError>>) {
    let hwnd = match create_window() {
        Ok(h) => h,
        Err(e) => {
            let _ = tx.send(Err(e));
            return;
        }
    };
    let worker = Rc::new(RefCell::new(worker));
    {
        let w = Rc::clone(&worker);
        HANDLER.with(|h| {
            *h.borrow_mut() = Some(Box::new(move |m, wp, lp| {
                w.borrow_mut().handle_message(m, wp, lp);
            }));
        });
    }
    let setup_result = worker.borrow_mut().setup(hwnd);
    let setup_ok = setup_result.is_ok();
    if tx.send(setup_result.map(|()| hwnd.0 as isize)).is_err() || !setup_ok {
        cleanup(hwnd, &worker);
        return;
    }
    let mut msg = MSG::default();
    loop {
        let r = unsafe { GetMessageW(&raw mut msg, None, 0, 0) };
        if r.0 <= 0 {
            break;
        }
        unsafe {
            let _ = TranslateMessage(&raw const msg);
            DispatchMessageW(&raw const msg);
        }
    }
    cleanup(hwnd, &worker);
}

fn cleanup<W: MessageWorker>(hwnd: HWND, worker: &Rc<RefCell<W>>) {
    // Teardown (unregister notifications) before clearing the handler and
    // destroying the window, so no notification fires into a dead handler.
    worker.borrow_mut().teardown();
    HANDLER.with(|h| *h.borrow_mut() = None);
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
}

fn create_window() -> Result<HWND, VolumeError> {
    unsafe {
        let hinstance = GetModuleHandleW(None)
            .map_err(|e| VolumeError::BackendInit(format!("GetModuleHandleW: {e}")))?;
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance.into(),
            lpszClassName: WINDOW_CLASS,
            ..Default::default()
        };
        // A zero return with ERROR_CLASS_ALREADY_EXISTS just means another
        // provider instance in this process registered it; CreateWindowExW
        // below is the real gate.
        let _ = RegisterClassW(&raw const wc);
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            WINDOW_CLASS,
            w!("yoke-volume"),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(hinstance.into()),
            None,
        )
        .map_err(|e| VolumeError::BackendInit(format!("CreateWindowExW: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct TrivialWorker {
        setup_called: Arc<AtomicBool>,
        teardown_called: Arc<AtomicBool>,
    }

    impl MessageWorker for TrivialWorker {
        fn setup(&mut self, _hwnd: HWND) -> Result<(), VolumeError> {
            self.setup_called.store(true, Ordering::SeqCst);
            Ok(())
        }
        fn handle_message(&mut self, _msg: u32, _wparam: WPARAM, _lparam: LPARAM) {}
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
        let thread = MessageWindowThread::spawn(worker).expect("spawn");
        assert!(
            setup.load(Ordering::SeqCst),
            "setup must run before spawn returns"
        );
        drop(thread);
        assert!(
            teardown.load(Ordering::SeqCst),
            "teardown must run after shutdown"
        );
    }

    struct FailingWorker;

    impl MessageWorker for FailingWorker {
        fn setup(&mut self, _hwnd: HWND) -> Result<(), VolumeError> {
            Err(VolumeError::BackendInit("test failure".into()))
        }
        fn handle_message(&mut self, _msg: u32, _wparam: WPARAM, _lparam: LPARAM) {}
        fn teardown(&mut self) {}
    }

    #[test]
    fn spawn_propagates_setup_failure() {
        match MessageWindowThread::spawn(FailingWorker) {
            Err(VolumeError::BackendInit(_)) => {}
            Err(other) => panic!("expected BackendInit, got {other:?}"),
            Ok(_) => panic!("expected setup failure to surface"),
        }
    }
}
