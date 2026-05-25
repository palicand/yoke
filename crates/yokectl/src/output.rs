use std::io::{self, Write};
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Human,
    Json,
}

pub struct Output {
    pub format: OutputFormat,
    stdout: Mutex<Box<dyn Write + Send>>,
    stderr: Mutex<Box<dyn Write + Send>>,
}

impl Output {
    pub const fn is_json(&self) -> bool {
        matches!(self.format, OutputFormat::Json)
    }

    pub fn from_flags(json: bool) -> Self {
        Self::with_writers(json, Box::new(io::stdout()), Box::new(io::stderr()))
    }

    pub fn with_writers(
        json: bool,
        stdout: Box<dyn Write + Send>,
        stderr: Box<dyn Write + Send>,
    ) -> Self {
        let format = if json {
            OutputFormat::Json
        } else {
            OutputFormat::Human
        };
        Self {
            format,
            stdout: Mutex::new(stdout),
            stderr: Mutex::new(stderr),
        }
    }

    #[allow(clippy::missing_panics_doc)]
    pub fn emit<S, F>(&self, value: &S, human: F) -> anyhow::Result<()>
    where
        S: serde::Serialize,
        F: FnOnce(&mut dyn Write) -> io::Result<()>,
    {
        match self.format {
            OutputFormat::Json => {
                let mut out = self.stdout.lock().expect("stdout mutex poisoned");
                serde_json::to_writer(&mut *out, value)?;
                writeln!(out)?;
            }
            OutputFormat::Human => {
                let mut out = self.stdout.lock().expect("stdout mutex poisoned");
                human(&mut *out)?;
            }
        }
        Ok(())
    }

    #[allow(clippy::missing_panics_doc, clippy::needless_pass_by_value)]
    pub fn emit_error(&self, code: &str, message: &str, details: serde_json::Value) {
        match self.format {
            OutputFormat::Json => {
                let env = serde_json::json!({
                    "error": { "code": code, "message": message, "details": details }
                });
                let _ = writeln!(self.stdout.lock().expect("stdout mutex poisoned"), "{env}");
            }
            OutputFormat::Human => {
                let _ = writeln!(
                    self.stderr.lock().expect("stderr mutex poisoned"),
                    "error: {message}"
                );
            }
        }
    }
}
