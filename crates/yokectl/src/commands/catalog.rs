use std::io::Write;

use anyhow::Result;
use yoke_config::catalog::{
    Channel, DPadDir, GamepadButton, JoyOutput, KbKey, MouseAction, MpPosition, PreferenceSpec,
    SipPuff, SubProfileMode, SystemAction, UsbHost,
};

use crate::output::Output;

pub fn run_inputs(out: &Output) -> Result<()> {
    let entries: Vec<String> = SipPuff::ALL
        .iter()
        .map(|v| v.as_csv().to_string())
        .chain(MpPosition::ALL.iter().map(|v| v.as_csv().to_string()))
        .chain(DPadDir::ALL.iter().map(|v| v.as_csv().to_string()))
        .chain(UsbHost::ALL.iter().map(|v| v.as_csv_index().to_string()))
        .collect();
    emit(out, "inputs", &entries)
}

pub fn run_outputs(out: &Output) -> Result<()> {
    let entries: Vec<String> = KbKey::ALL
        .iter()
        .map(|v| v.as_csv().to_string())
        .chain(MouseAction::ALL.iter().map(|v| v.as_csv().to_string()))
        .chain(GamepadButton::ALL.iter().map(|v| v.as_csv().to_string()))
        .chain(JoyOutput::ALL.iter().map(|v| v.as_csv().to_string()))
        .chain(SystemAction::ALL.iter().map(|v| v.as_csv().to_string()))
        .chain(std::iter::once("touch".to_string()))
        .collect();
    emit(out, "outputs", &entries)
}

pub fn run_preferences(out: &Output) -> Result<()> {
    let entries: Vec<String> = PreferenceSpec::ALL
        .iter()
        .map(|s| s.id.to_string())
        .collect();
    emit(out, "preferences", &entries)
}

pub fn run_modes(out: &Output) -> Result<()> {
    let entries: Vec<String> = SubProfileMode::KNOWN
        .iter()
        .map(SubProfileMode::canonical_csv)
        .collect();
    emit(out, "modes", &entries)
}

pub fn run_channels(out: &Output) -> Result<()> {
    let entries: Vec<String> = Channel::ALL
        .iter()
        .map(|c| c.canonical_csv().to_string())
        .collect();
    emit(out, "channels", &entries)
}

fn emit(out: &Output, key: &str, entries: &[String]) -> Result<()> {
    out.emit(&serde_json::json!({ key: entries }), |w| {
        for e in entries {
            writeln!(w, "{e}")?;
        }
        Ok(())
    })
}
