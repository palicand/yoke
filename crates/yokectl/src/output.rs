use std::io::{Stdout, Write};

use is_terminal::IsTerminal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Human,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    On,
    Off,
}

#[allow(dead_code)]
pub struct Output {
    pub format: OutputFormat,
    pub color: ColorMode,
}

#[allow(dead_code)]
impl Output {
    pub const fn is_json(&self) -> bool {
        matches!(self.format, OutputFormat::Json)
    }

    pub fn from_flags(json: bool, no_color: bool) -> Self {
        let env_no_color = std::env::var_os("NO_COLOR").is_some();
        let stdout_is_tty = std::io::stdout().is_terminal();
        let color = if no_color || env_no_color || !stdout_is_tty {
            ColorMode::Off
        } else {
            ColorMode::On
        };
        let format = if json {
            OutputFormat::Json
        } else {
            OutputFormat::Human
        };
        Self { format, color }
    }

    pub fn emit<S: serde::Serialize, F: FnOnce(&mut Stdout) -> std::io::Result<()>>(
        &self,
        value: &S,
        human: F,
    ) -> anyhow::Result<()> {
        let mut out = std::io::stdout();
        match self.format {
            OutputFormat::Json => {
                serde_json::to_writer(&mut out, value)?;
                writeln!(out)?;
            }
            OutputFormat::Human => human(&mut out)?,
        }
        Ok(())
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn emit_error(&self, code: &str, message: &str, details: serde_json::Value) {
        match self.format {
            OutputFormat::Json => {
                let env = serde_json::json!({
                    "error": { "code": code, "message": message, "details": details }
                });
                println!("{env}");
            }
            OutputFormat::Human => {
                let mut stderr = std::io::stderr();
                let _ = writeln!(stderr, "error: {message}");
            }
        }
    }
}
