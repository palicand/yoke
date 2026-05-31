use anyhow::Result;
use yoke_config::catalog::{Channel, Input, Modifier, Output, PreferenceSpec, SubProfileMode};

use crate::output::Output as CliOutput;

pub fn run_inputs(out: &CliOutput) -> Result<()> {
    let entries: Vec<String> = Input::all_csv_names().collect();
    emit(out, "inputs", &entries)
}

pub fn run_outputs(out: &CliOutput) -> Result<()> {
    let entries: Vec<String> = Output::all_csv_names().collect();
    emit(out, "outputs", &entries)
}

pub fn run_preferences(out: &CliOutput) -> Result<()> {
    let entries: Vec<String> = PreferenceSpec::ALL
        .iter()
        .map(|s| s.id.to_string())
        .collect();
    emit(out, "preferences", &entries)
}

pub fn run_modes(out: &CliOutput) -> Result<()> {
    let entries: Vec<String> = SubProfileMode::KNOWN
        .iter()
        .map(SubProfileMode::canonical_csv)
        .collect();
    emit(out, "modes", &entries)
}

pub fn run_channels(out: &CliOutput) -> Result<()> {
    let entries: Vec<String> = Channel::ALL
        .iter()
        .map(|c| c.canonical_csv().to_string())
        .collect();
    emit(out, "channels", &entries)
}

pub fn run_modifiers(out: &CliOutput) -> Result<()> {
    let entries: Vec<String> = Modifier::KEYWORDS
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    emit(out, "modifiers", &entries)
}

fn emit(out: &CliOutput, key: &str, entries: &[String]) -> Result<()> {
    out.emit(&serde_json::json!({ key: entries }), |w| {
        for e in entries {
            writeln!(w, "{e}")?;
        }
        Ok(())
    })
}
