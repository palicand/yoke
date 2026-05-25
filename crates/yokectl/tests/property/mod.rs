#![allow(dead_code)]

use std::io::Write;
use std::sync::Arc;
use tempfile::TempDir;
use yoke_volume::VolumeProvider;
use yokectl::cli::{Cli, Commands};
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
pub fn dispatch_in_process(cli: Cli, provider: &Arc<dyn VolumeProvider>) -> Capture {
    use std::sync::Mutex;
    let stdout = Arc::new(Mutex::new(Vec::<u8>::new()));
    let stderr = Arc::new(Mutex::new(Vec::<u8>::new()));
    let stdout_writer = Box::new(SharedBuffer(stdout.clone())) as Box<dyn Write + Send>;
    let stderr_writer = Box::new(SharedBuffer(stderr.clone())) as Box<dyn Write + Send>;
    let json = cli.json;
    let out = Output::with_writers(json, stdout_writer, stderr_writer);

    let result = yokectl::run_with_provider(cli, &out, provider);
    let code = match result {
        Ok(()) => 0,
        Err(err) => {
            let info = yokectl::error::classify(&err);
            out.emit_error(&info.envelope_code, &err.to_string(), info.details);
            info.code
        }
    };
    drop(out);
    let stdout_bytes = std::sync::Arc::try_unwrap(stdout)
        .unwrap_or_else(|a| Mutex::new(a.lock().unwrap().clone()))
        .into_inner()
        .unwrap();
    let stderr_bytes = std::sync::Arc::try_unwrap(stderr)
        .unwrap_or_else(|a| Mutex::new(a.lock().unwrap().clone()))
        .into_inner()
        .unwrap();
    Capture {
        code,
        stdout: stdout_bytes,
        stderr: stderr_bytes,
    }
}

struct SharedBuffer(Arc<std::sync::Mutex<Vec<u8>>>);

impl Write for SharedBuffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

use proptest::prelude::*;
use yoke_config::catalog::{Channel, SubProfileMode};
use yoke_edit::{EditOp, PreferenceValue};
use yoke_index::ProfileSource;

#[derive(Debug, Clone)]
pub enum Action {
    SetTitle {
        target: String,
        title: String,
    },
    SetPreference {
        target: String,
        key: String,
        value: PreferenceValue,
    },
    UnsetPreference {
        target: String,
        key: String,
    },
    SetBinding {
        target: String,
        sub_profile: String,
        input: String,
        output: String,
    },
    ClearBinding {
        target: String,
        sub_profile: String,
        input: String,
    },
    SetOverride {
        target: String,
        sub_profile: String,
        key: String,
        value: PreferenceValue,
    },
    UnsetOverride {
        target: String,
        sub_profile: String,
        key: String,
    },
    AddSubProfile {
        target: String,
        name: String,
        mode: SubProfileMode,
        channel: Channel,
        sub_mode: Option<String>,
    },
    DeleteSubProfile {
        target: String,
        name: String,
    },
    RenameSubProfile {
        target: String,
        from: String,
        to: String,
    },
    CloneSubProfile {
        target: String,
        from: String,
        to: String,
    },
    Push {
        name: String,
        bytes: Vec<u8>,
    },
    Pull {
        name: String,
    },
    Copy {
        from: String,
        to: String,
    },
    Rename {
        from: String,
        to: String,
    },
    Delete {
        name: String,
        force: bool,
    },
    Show {
        target: String,
        raw: bool,
    },
    Validate {
        target: String,
    },
    Bindings {
        target: String,
        sub_profile: Option<String>,
    },
    Preferences {
        target: String,
        sub_profile: Option<String>,
        raw: bool,
    },
    Apply {
        target: String,
        ops: Vec<EditOp>,
    },
    Install {
        source: ProfileSource,
    },
    CatalogInputs,
    CatalogOutputs,
    CatalogPreferences,
    CatalogModes,
    CatalogChannels,
    Device,
    List,
}

pub fn action_strategy(seed_names: &[String]) -> impl Strategy<Value = Action> {
    let name_or_unknown = if seed_names.is_empty() {
        Just(String::from("unknown")).boxed()
    } else {
        prop::sample::select(seed_names.to_owned()).boxed()
    };
    let inputs: Vec<String> = yoke_config::catalog::Input::all_csv_names().collect();
    let outputs: Vec<String> = yoke_config::catalog::Output::all_csv_names().collect();
    let pref_keys: Vec<String> = yoke_config::catalog::PreferenceSpec::ALL
        .iter()
        .map(|s| s.id.to_string())
        .collect();
    let input_strat = prop::sample::select(inputs);
    let output_strat = prop::sample::select(outputs);
    let pref_key_strat = prop::sample::select(pref_keys);
    let pref_value = prop_oneof![
        Just(PreferenceValue::Bool(true)),
        Just(PreferenceValue::Bool(false)),
        (0i64..=100).prop_map(PreferenceValue::Number),
        "[a-zA-Z]{1,8}".prop_map(PreferenceValue::Text),
    ];

    prop_oneof![
        (name_or_unknown.clone(), input_strat, output_strat).prop_map(|(t, i, o)| {
            Action::SetBinding {
                target: t,
                sub_profile: "Main".into(),
                input: i,
                output: o,
            }
        }),
        (
            name_or_unknown.clone(),
            pref_key_strat.clone(),
            pref_value.clone()
        )
            .prop_map(|(t, k, v)| Action::SetPreference {
                target: t,
                key: k,
                value: v
            }),
        (name_or_unknown.clone(), pref_key_strat)
            .prop_map(|(t, k)| Action::UnsetPreference { target: t, key: k }),
        Just(Action::List),
        Just(Action::CatalogInputs),
        Just(Action::CatalogChannels),
        name_or_unknown.clone().prop_map(|t| Action::Show {
            target: t,
            raw: false
        }),
        name_or_unknown.clone().prop_map(|t| Action::Bindings {
            target: t,
            sub_profile: None
        }),
        name_or_unknown.prop_map(|t| Action::Preferences {
            target: t,
            sub_profile: None,
            raw: false
        }),
    ]
}

pub fn action_to_cli(action: &Action, base: &Cli) -> Cli {
    let mut cli = base.clone();
    cli.command = match action {
        Action::List => Commands::List,
        Action::Device => Commands::Device,
        Action::Show { target, raw } => Commands::Show {
            target: target.clone(),
            raw: *raw,
        },
        Action::Validate { target } => Commands::Validate {
            target: target.clone(),
        },
        Action::Bindings {
            target,
            sub_profile,
        } => Commands::Bindings {
            target: target.clone(),
            sub_profile: sub_profile.clone(),
        },
        Action::Preferences {
            target,
            sub_profile,
            raw,
        } => Commands::Preferences {
            target: target.clone(),
            sub_profile: sub_profile.clone(),
            raw: *raw,
        },
        Action::SetBinding {
            target,
            sub_profile,
            input,
            output,
        } => Commands::SetBinding {
            target: target.clone(),
            sub_profile: sub_profile.clone(),
            input: input.clone(),
            output: output.clone(),
        },
        Action::SetPreference { target, key, value } => Commands::SetPreference {
            target: target.clone(),
            key: key.clone(),
            value: match value {
                PreferenceValue::Bool(b) => b.to_string(),
                PreferenceValue::Number(n) => n.to_string(),
                PreferenceValue::Text(s) => s.clone(),
            },
        },
        Action::UnsetPreference { target, key } => Commands::UnsetPreference {
            target: target.clone(),
            key: key.clone(),
        },
        Action::CatalogInputs => Commands::Catalog {
            cmd: yokectl::cli::CatalogCmd::Inputs,
        },
        Action::CatalogOutputs => Commands::Catalog {
            cmd: yokectl::cli::CatalogCmd::Outputs,
        },
        Action::CatalogPreferences => Commands::Catalog {
            cmd: yokectl::cli::CatalogCmd::Preferences,
        },
        Action::CatalogModes => Commands::Catalog {
            cmd: yokectl::cli::CatalogCmd::Modes,
        },
        Action::CatalogChannels => Commands::Catalog {
            cmd: yokectl::cli::CatalogCmd::Channels,
        },
        other => panic!("action_to_cli: variant not yet mapped: {other:?}"),
    };
    cli
}

pub fn seed_tempdir(seed_csvs: &[(&str, &str)]) -> (TempDir, Arc<dyn VolumeProvider>) {
    let dir = tempfile::tempdir().unwrap();
    for (name, body) in seed_csvs {
        std::fs::write(dir.path().join(name), body).unwrap();
    }
    let provider: Arc<dyn VolumeProvider> = Arc::new(yoke_volume::fs_backend::FsBackend::new(
        dir.path().to_path_buf(),
    ));
    (dir, provider)
}

pub mod apply_atomicity;
pub mod exit_and_json;
pub mod no_panics;
pub mod round_trip;
