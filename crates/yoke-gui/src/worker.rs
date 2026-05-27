use crate::data::{handle_command, AppCommand, DataEvent, DataSource, FailureContext};

/// Drive one command synchronously and append the resulting event(s).
/// `OpenFileDialog` is special-cased per target (see `WorkerHandle`).
pub fn pump_inline(data: &dyn DataSource, cmd: AppCommand, out: &mut Vec<DataEvent>) {
    out.push(handle_command(data, cmd));
}

#[cfg(not(target_arch = "wasm32"))]
mod native_worker {
    use std::sync::mpsc::{Receiver, Sender};
    use std::sync::Arc;

    use super::{AppCommand, DataEvent, FailureContext};
    use crate::data::{handle_command, DataSource};
    use crate::data::native::NativeDataSource;
    use crate::state::ProfileSource;

    pub struct WorkerHandle {
        cmd_tx: Sender<AppCommand>,
    }

    impl WorkerHandle {
        pub fn send(&self, cmd: AppCommand) {
            let _ = self.cmd_tx.send(cmd);
        }
    }

    /// Spawn the worker thread + volume watcher. Returns the handle and the
    /// event receiver the UI drains each frame. `ctx` is used only to wake the
    /// UI; it never enters `data`/`state`.
    ///
    /// # Panics
    /// Panics if the tokio current-thread runtime for the volume watcher cannot
    /// be built (OS resource exhaustion).
    pub fn spawn(data: Arc<NativeDataSource>, ctx: egui::Context) -> (WorkerHandle, Receiver<DataEvent>) {
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

        // Command loop.
        {
            std::thread::spawn(move || {
                while let Ok(cmd) = cmd_rx.recv() {
                    let event = match cmd {
                        AppCommand::OpenFileDialog => open_file_dialog(data.as_ref()),
                        other => handle_command(data.as_ref(), other),
                    };
                    if evt_tx.send(event).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            });
        }

        (WorkerHandle { cmd_tx }, evt_rx)
    }

    fn open_file_dialog(data: &NativeDataSource) -> DataEvent {
        // rfd's sync dialog blocks this worker thread; on macOS rfd dispatches
        // NSOpenPanel to the main queue internally, so this is safe off-main.
        let Some(path) = rfd::FileDialog::new().add_filter("CSV", &["csv"]).pick_file() else {
            // Cancelled: emit a benign no-op event the UI ignores.
            return DataEvent::Failed { context: FailureContext::OpenFile, message: String::new() };
        };
        match data.read_file_profile(&path) {
            Ok(profile) => DataEvent::ProfileOpened {
                source: ProfileSource::File(path),
                profile: Box::new(profile),
            },
            Err(e) => DataEvent::Failed { context: FailureContext::OpenFile, message: e.to_string() },
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native_worker::{spawn, WorkerHandle};

#[cfg(target_arch = "wasm32")]
mod wasm_worker {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::{pump_inline, AppCommand, DataEvent};
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
pub use wasm_worker::{spawn, WorkerHandle};

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
