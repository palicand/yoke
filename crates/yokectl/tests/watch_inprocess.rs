use std::sync::{Arc, Mutex};
use std::time::Duration;

use tempfile::tempdir;
use yoke_volume::fs_backend::FsBackend;
use yoke_volume::state::{MountEvent, MountState};

// Adapter that lets the watch loop write into the shared buffer while we
// continue to drive the backend from this task; std::io::Cursor is not
// Sync, so a Mutex-wrapped Vec keeps the writer side single-owned.
struct SharedWriter(Arc<Mutex<Vec<u8>>>);
impl std::io::Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[tokio::test(flavor = "current_thread")]
async fn simulated_events_arrive_on_watch_stream() {
    let dir = tempdir().unwrap();
    let backend = Arc::new(FsBackend::new(dir.path().to_path_buf()));
    let captured: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));

    let backend_for_task = backend.clone();
    let writer = SharedWriter(captured.clone());
    let handle = tokio::spawn(async move {
        // We deliberately ignore the return value; the test drives the loop
        // and aborts the spawn instead of relying on watch_json to terminate.
        let _ = yokectl::commands::watch::watch_json(backend_for_task, writer).await;
    });

    let backend_for_wait = backend.clone();
    tokio::time::timeout(Duration::from_secs(2), async {
        while backend_for_wait.event_subscriber_count() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("watch loop never subscribed to events");
    backend.simulate_event(MountEvent::DeviceDisappeared);
    backend.simulate_state(&MountState::Absent);
    tokio::time::sleep(Duration::from_millis(50)).await;
    handle.abort();

    let bytes = captured.lock().unwrap().clone();
    let s = String::from_utf8(bytes).unwrap();
    assert!(
        s.contains("DeviceDisappeared"),
        "expected event in stream, got: {s}"
    );
    assert!(
        s.contains("mount-state"),
        "expected state line in stream, got: {s}"
    );
}
