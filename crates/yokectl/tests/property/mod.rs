#![allow(dead_code)]

use std::io::Write;
use std::sync::Arc;
use yoke_volume::VolumeProvider;
use yokectl::cli::Cli;
use yokectl::output::Output;

/// One captured invocation: exit code (mapped from `anyhow::Error`), stdout bytes, stderr bytes.
pub struct Capture {
    pub code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// In-process dispatch entry point used by every proptest invariant.
/// Equivalent to `yokectl::entry()` but captures output into Vecs and
/// returns the exit info instead of calling `process::exit`.
pub fn dispatch_in_process(cli: Cli, provider: Arc<dyn VolumeProvider>) -> Capture {
    let stdout_buf: Vec<u8> = Vec::new();
    let stderr_buf: Vec<u8> = Vec::new();
    let stdout_writer: Box<dyn Write + Send> = Box::new(InspectableBuffer::default());
    let stderr_writer: Box<dyn Write + Send> = Box::new(InspectableBuffer::default());
    let out = Output::with_writers(cli.json, stdout_writer, stderr_writer);
    let _ = (provider, &out, cli, stdout_buf, stderr_buf);
    todo!()
}

#[derive(Default)]
struct InspectableBuffer(Arc<std::sync::Mutex<Vec<u8>>>);

impl Write for InspectableBuffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
