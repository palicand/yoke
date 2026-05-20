use std::io::{Stdout, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Human,
    Json,
}

pub struct Output {
    pub format: OutputFormat,
}

impl Output {
    pub fn from_flags(json: bool, no_color: bool) -> Self {
        let format = if json {
            OutputFormat::Json
        } else {
            OutputFormat::Human
        };
        // Disable colors immediately when --no-color is passed; honoring NO_COLOR and
        // is_attended() is left to console's defaults when --no-color is absent.
        if no_color {
            console::set_colors_enabled(false);
            console::set_colors_enabled_stderr(false);
        }
        Self { format }
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
                let _ = writeln!(std::io::stdout(), "{env}");
            }
            OutputFormat::Human => {
                let _ = writeln!(std::io::stderr(), "error: {message}");
            }
        }
    }
}
