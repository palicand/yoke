use crate::data::{AppCommand, DataEvent, DataSource, handle_command};

/// Drive one command synchronously and append the resulting event(s).
/// `OpenFileDialog` is special-cased per target (see `WorkerHandle`).
pub fn pump_inline(data: &dyn DataSource, cmd: AppCommand, out: &mut Vec<DataEvent>) {
    out.push(handle_command(data, cmd));
}

#[cfg(not(target_arch = "wasm32"))]
mod native_worker {
    use std::sync::mpsc::{Receiver, Sender};
    use std::sync::{Arc, Mutex};

    use super::{AppCommand, DataEvent};
    use crate::data::native::NativeDataSource;
    use crate::data::{DataSource, FailureContext, handle_command};
    use crate::state::ProfileSource;

    // Fixed worker-pool size. Up to this many commands run concurrently (so a
    // slow community fetch never blocks a queued device/file open), while excess
    // commands wait in the channel instead of spawning unbounded OS threads on
    // rapid clicks.
    const WORKER_THREADS: usize = 4;

    pub struct WorkerHandle {
        cmd_tx: Sender<AppCommand>,
    }

    impl WorkerHandle {
        pub fn send(&self, cmd: AppCommand) {
            if let Err(err) = self.cmd_tx.send(cmd) {
                tracing::error!(?err, "failed to send gui command to worker");
            }
        }
    }

    #[cfg(test)]
    impl WorkerHandle {
        // A handle whose receiver is dropped; sends fail (and log) but never
        // panic. Lets `app` tests construct a `YokeApp` without a live worker.
        pub(crate) fn for_test() -> Self {
            let (cmd_tx, _rx) = std::sync::mpsc::channel();
            Self { cmd_tx }
        }
    }

    /// Spawn the worker thread + volume watcher. Returns the handle and the
    /// event receiver the UI drains each frame. `ctx` is used only to wake the
    /// UI; it never enters `data`/`state`.
    ///
    /// # Panics
    /// Panics if the tokio current-thread runtime for the volume watcher cannot
    /// be built (OS resource exhaustion).
    pub fn spawn(
        data: &Arc<NativeDataSource>,
        ctx: &egui::Context,
    ) -> (WorkerHandle, Receiver<DataEvent>) {
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<AppCommand>();
        let (evt_tx, evt_rx) = std::sync::mpsc::channel::<DataEvent>();

        // Volume watcher: forwards MountState changes + wakes the UI.
        {
            let mut rx = data.subscribe_state();
            let evt_tx = evt_tx.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                // watch::Receiver::changed is async, so drive it on a small
                // current-thread runtime dedicated to this watcher.
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("watcher runtime");
                rt.block_on(async move {
                    loop {
                        let state = rx.borrow().clone();
                        if evt_tx.send(DataEvent::VolumeChanged(state)).is_err() {
                            break;
                        }
                        ctx.request_repaint();
                        if rx.changed().await.is_err() {
                            break;
                        }
                    }
                });
            });
        }

        // Command dispatcher: a fixed pool of worker threads drains the shared
        // command channel. Each thread holds the receiver lock only across
        // `recv()` (released before the potentially-slow work runs), so up to
        // WORKER_THREADS commands run concurrently while a slow network call
        // (community list/fetch, which `block_on`s inside the data source) never
        // blocks a device or file command — and rapid clicks queue in the channel
        // rather than spawning unbounded threads. The data source is shared (Arc)
        // and owns a multi-thread runtime, so concurrent `block_on` calls from
        // separate pool threads are safe.
        let cmd_rx = Arc::new(Mutex::new(cmd_rx));
        for _ in 0..WORKER_THREADS {
            let cmd_rx = Arc::clone(&cmd_rx);
            let data = Arc::clone(data);
            let evt_tx = evt_tx.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                loop {
                    let cmd = {
                        let rx = cmd_rx.lock().unwrap();
                        rx.recv()
                    };
                    let Ok(cmd) = cmd else { break };
                    let event = match cmd {
                        AppCommand::OpenFileDialog { req } => open_file_dialog(data.as_ref(), req),
                        AppCommand::SaveAsDialog {
                            req,
                            bytes,
                            file_name,
                        } => save_as_dialog(data.as_ref(), req, &bytes, &file_name),
                        other => handle_command(data.as_ref(), other),
                    };
                    let _ = evt_tx.send(event);
                    ctx.request_repaint();
                }
            });
        }

        (WorkerHandle { cmd_tx }, evt_rx)
    }

    fn open_file_dialog(data: &NativeDataSource, req: u64) -> DataEvent {
        // rfd's sync dialog blocks this worker thread; on macOS rfd dispatches
        // NSOpenPanel to the main queue internally, so this is safe off-main.
        let Some(path) = rfd::FileDialog::new()
            .add_filter("CSV", &["csv"])
            .pick_file()
        else {
            return DataEvent::FileDialogCancelled { req };
        };
        match data.read_file_profile(&path) {
            Ok(profile_result) => DataEvent::ProfileOpened {
                req,
                source: ProfileSource::File(path),
                parsed: Box::new(profile_result),
            },
            Err(e) => DataEvent::Failed {
                req: Some(req),
                context: FailureContext::OpenFile,
                message: e.to_string(),
            },
        }
    }

    fn save_as_dialog(
        data: &NativeDataSource,
        req: u64,
        bytes: &[u8],
        file_name: &str,
    ) -> DataEvent {
        // rfd's sync dialog blocks this worker thread; safe off-main on macOS.
        let Some(path) = rfd::FileDialog::new()
            .add_filter("CSV", &["csv"])
            .set_file_name(file_name)
            .save_file()
        else {
            return DataEvent::FileDialogCancelled { req };
        };
        match data.write_file_profile(&path, bytes) {
            Ok(()) => DataEvent::Saved {
                req,
                label: path.display().to_string(),
            },
            Err(e) => DataEvent::Failed {
                req: Some(req),
                context: FailureContext::SaveFile,
                message: e.to_string(),
            },
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native_worker::{WorkerHandle, spawn};

#[cfg(target_arch = "wasm32")]
mod wasm_worker {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::{AppCommand, DataEvent, pump_inline};
    use crate::data::mock::MockDataSource;

    pub struct WorkerHandle {
        data: Rc<MockDataSource>,
        queue: Rc<RefCell<Vec<DataEvent>>>,
    }

    impl WorkerHandle {
        pub fn send(&self, cmd: AppCommand) {
            // Mock data is in-memory: complete synchronously into the queue.
            pump_inline(self.data.as_ref(), cmd, &mut self.queue.borrow_mut());
        }

        pub fn drain(&self) -> Vec<DataEvent> {
            std::mem::take(&mut self.queue.borrow_mut())
        }
    }

    pub fn spawn(data: Rc<MockDataSource>) -> WorkerHandle {
        let queue = Rc::new(RefCell::new(Vec::new()));
        // Emit the startup volume state once.
        queue.borrow_mut().push(DataEvent::VolumeChanged(
            <MockDataSource as crate::data::DataSource>::volume_state(data.as_ref()),
        ));
        WorkerHandle { data, queue }
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm_worker::{WorkerHandle, spawn};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::mock::MockDataSource;

    #[test]
    fn inline_pump_lists_profiles() {
        let data = MockDataSource::new();
        let mut out: Vec<DataEvent> = Vec::new();
        pump_inline(&data, AppCommand::ListDeviceProfiles, &mut out);
        assert!(matches!(out.as_slice(), [DataEvent::ProfilesListed(_)]));
    }
}
