#![allow(dead_code, clippy::too_many_lines)]

use std::io::Write;
use std::path::Path;
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

/// In-process dispatch used by every proptest invariant. Drives
/// `yokectl::run_with_provider` with an injected provider and captures stdout/stderr
/// into buffers instead of calling `process::exit`. It deliberately does not cover
/// `yokectl::run`'s provider selection (dummy vs platform/`--fake-volume` backend) or
/// `entry()`'s argument parsing; those layers are exercised by the command-level
/// integration tests that spawn the real binary.
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
use yoke_config::catalog::{Channel, Modifier, SubProfileMode};
use yoke_edit::{EditOp, PreferenceValue};
use yoke_index::ProfileSource;

/// Seed profile shared by the property invariants. It carries a real `Main` sub-profile
/// at index 0 with one binding (`left -> mouse_left [normal]`), so the sub-profile-scoped
/// edit ops (add/update/clear binding, overrides) actually reach their success and
/// not-found branches during fuzzing rather than bailing out at `SubProfileIndexOutOfRange`.
pub const SEED: &str = "QuadStick Configuration,Version 1.4,Mock,Default\r\n\
Profile Name,Main,Mouse,usb\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mouse_left,normal,left,\r\n\
\r\n";

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
    AddBinding {
        target: String,
        sub_profile: usize,
        input: String,
        output: String,
        modifier: Option<String>,
    },
    UpdateBinding {
        target: String,
        sub_profile: usize,
        input: String,
        output: String,
        modifier: String,
    },
    ClearBinding {
        target: String,
        sub_profile: usize,
        input: String,
        modifier: Option<String>,
    },
    SetOverride {
        target: String,
        sub_profile: usize,
        key: String,
        value: PreferenceValue,
    },
    UnsetOverride {
        target: String,
        sub_profile: usize,
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
        index: usize,
    },
    RenameSubProfile {
        target: String,
        index: usize,
        to: String,
    },
    CloneSubProfile {
        target: String,
        index: usize,
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
    CatalogModifiers,
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
    let modes: Vec<SubProfileMode> = SubProfileMode::KNOWN.to_vec();
    let channels: Vec<Channel> = Channel::ALL.to_vec();
    let input_strat = prop::sample::select(inputs);
    let output_strat = prop::sample::select(outputs);
    let pref_key_strat = prop::sample::select(pref_keys);
    let mode_strat = prop::sample::select(modes);
    let channel_strat = prop::sample::select(channels);
    let pref_value = prop_oneof![
        Just(PreferenceValue::Bool(true)),
        Just(PreferenceValue::Bool(false)),
        (0i64..=100).prop_map(PreferenceValue::Number),
        "[a-zA-Z]{1,8}".prop_map(PreferenceValue::Text),
    ];

    // EditOp strategy for Apply
    let edit_input_strat =
        prop::sample::select(yoke_config::catalog::Input::all_csv_names().collect::<Vec<_>>());
    let edit_output_strat =
        prop::sample::select(yoke_config::catalog::Output::all_csv_names().collect::<Vec<_>>());
    let edit_pref_key_strat = prop::sample::select(
        yoke_config::catalog::PreferenceSpec::ALL
            .iter()
            .map(|s| s.id.to_string())
            .collect::<Vec<_>>(),
    );
    let edit_pref_value = prop_oneof![
        Just(PreferenceValue::Bool(true)),
        Just(PreferenceValue::Bool(false)),
        (0i64..=100).prop_map(PreferenceValue::Number),
        "[a-zA-Z]{1,8}".prop_map(PreferenceValue::Text),
    ];
    // Modifier phrases: a keyword optionally followed by a numeric arg. u8-typed
    // modifiers (greater_than/less_than) given >255 round-trip to Unknown and are
    // rejected by apply; u32-typed modifiers accept the whole range. A fresh strategy is
    // built per use (`mod_phrase()`) because proptest combinators are not `Clone`.
    let modifier_keywords: Vec<String> = Modifier::KEYWORDS
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    let mod_phrase = || {
        (
            prop::sample::select(modifier_keywords.clone()),
            prop::option::of(0u32..1000),
        )
            .prop_map(|(kw, arg)| match arg {
                Some(n) => format!("{kw} {n}"),
                None => kw,
            })
    };
    let edit_op_strat = prop_oneof![
        "[a-zA-Z]{1,8}".prop_map(|title| EditOp::SetTitle { title }),
        (edit_pref_key_strat.clone(), edit_pref_value.clone())
            .prop_map(|(key, value)| EditOp::SetPreference { key, value }),
        edit_pref_key_strat
            .clone()
            .prop_map(|key| EditOp::UnsetPreference { key }),
        (
            edit_input_strat.clone(),
            edit_output_strat.clone(),
            prop::option::of(mod_phrase()),
        )
            .prop_map(|(input, output, modifier)| EditOp::AddBinding {
                sub_profile: 0,
                input,
                output,
                modifier,
            }),
        (edit_input_strat.clone(), edit_output_strat, mod_phrase()).prop_map(
            |(input, output, modifier)| EditOp::UpdateBinding {
                sub_profile: 0,
                input,
                output,
                modifier,
            },
        ),
        (edit_input_strat, prop::option::of(mod_phrase())).prop_map(|(input, modifier)| {
            EditOp::ClearBinding {
                sub_profile: 0,
                input,
                modifier,
            }
        }),
        (edit_pref_key_strat.clone(), edit_pref_value.clone()).prop_map(|(key, value)| {
            EditOp::SetOverride {
                sub_profile: 0,
                key,
                value,
            }
        }),
        edit_pref_key_strat.prop_map(|key| EditOp::UnsetOverride {
            sub_profile: 0,
            key,
        }),
        (0usize..4).prop_map(|index| EditOp::DeleteSubProfile { index }),
        (0usize..4, "[a-zA-Z]{1,8}").prop_map(|(index, to)| EditOp::RenameSubProfile { index, to }),
        (0usize..4, "[a-zA-Z]{1,8}").prop_map(|(index, to)| EditOp::CloneSubProfile { index, to }),
    ];

    let push_bytes = Just(
        b"QuadStick Configuration,Version 1.4,Mock,PushTest\r\n,,,\r\n*Main,sip_puff,,A\r\n"
            .to_vec(),
    );

    prop_oneof![
        // --- read-only / catalog ---
        Just(Action::List),
        Just(Action::Device),
        Just(Action::CatalogInputs),
        Just(Action::CatalogOutputs),
        Just(Action::CatalogPreferences),
        Just(Action::CatalogModes),
        Just(Action::CatalogChannels),
        Just(Action::CatalogModifiers),
        // --- profile read ---
        name_or_unknown.clone().prop_map(|t| Action::Show {
            target: t,
            raw: false
        }),
        name_or_unknown.clone().prop_map(|t| Action::Show {
            target: t,
            raw: true
        }),
        name_or_unknown
            .clone()
            .prop_map(|t| Action::Validate { target: t }),
        name_or_unknown.clone().prop_map(|t| Action::Bindings {
            target: t,
            sub_profile: None
        }),
        name_or_unknown.clone().prop_map(|t| Action::Preferences {
            target: t,
            sub_profile: None,
            raw: false
        }),
        name_or_unknown.clone().prop_map(|t| Action::Preferences {
            target: t,
            sub_profile: None,
            raw: true
        }),
        // --- profile write ---
        name_or_unknown
            .clone()
            .prop_map(|name| Action::Pull { name }),
        (name_or_unknown.clone(), push_bytes)
            .prop_map(|(name, bytes)| Action::Push { name, bytes }),
        (name_or_unknown.clone(), name_or_unknown.clone())
            .prop_map(|(from, to)| Action::Copy { from, to }),
        (name_or_unknown.clone(), name_or_unknown.clone())
            .prop_map(|(from, to)| Action::Rename { from, to }),
        name_or_unknown
            .clone()
            .prop_map(|name| Action::Delete { name, force: true }),
        // --- edit single-op ---
        (name_or_unknown.clone(), "[a-zA-Z]{1,8}")
            .prop_map(|(t, title)| Action::SetTitle { target: t, title }),
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
        (name_or_unknown.clone(), pref_key_strat.clone())
            .prop_map(|(t, k)| Action::UnsetPreference { target: t, key: k }),
        (
            name_or_unknown.clone(),
            input_strat.clone(),
            output_strat.clone(),
            prop::option::of(mod_phrase()),
        )
            .prop_map(|(t, i, o, m)| Action::AddBinding {
                target: t,
                sub_profile: 0,
                input: i,
                output: o,
                modifier: m,
            }),
        (
            name_or_unknown.clone(),
            input_strat.clone(),
            output_strat,
            mod_phrase(),
        )
            .prop_map(|(t, i, o, m)| Action::UpdateBinding {
                target: t,
                sub_profile: 0,
                input: i,
                output: o,
                modifier: m,
            }),
        (
            name_or_unknown.clone(),
            input_strat,
            prop::option::of(mod_phrase())
        )
            .prop_map(|(t, i, m)| Action::ClearBinding {
                target: t,
                sub_profile: 0,
                input: i,
                modifier: m,
            }),
        (
            name_or_unknown.clone(),
            pref_key_strat.clone(),
            pref_value.clone()
        )
            .prop_map(|(t, k, v)| Action::SetOverride {
                target: t,
                sub_profile: 0,
                key: k,
                value: v
            }),
        (name_or_unknown.clone(), pref_key_strat).prop_map(|(t, k)| Action::UnsetOverride {
            target: t,
            sub_profile: 0,
            key: k
        }),
        // --- sub-profile management ---
        (
            name_or_unknown.clone(),
            "[a-zA-Z]{1,8}",
            mode_strat,
            channel_strat
        )
            .prop_map(|(t, name, mode, channel)| Action::AddSubProfile {
                target: t,
                name,
                mode,
                channel,
                sub_mode: None,
            }),
        (name_or_unknown.clone(), 0usize..4)
            .prop_map(|(t, index)| Action::DeleteSubProfile { target: t, index }),
        (name_or_unknown.clone(), 0usize..4, "[a-zA-Z]{1,8}").prop_map(|(t, index, to)| {
            Action::RenameSubProfile {
                target: t,
                index,
                to,
            }
        }),
        (name_or_unknown.clone(), 0usize..4, "[a-zA-Z]{1,8}").prop_map(|(t, index, to)| {
            Action::CloneSubProfile {
                target: t,
                index,
                to,
            }
        }),
        // --- batch apply ---
        (
            name_or_unknown.clone(),
            prop::collection::vec(edit_op_strat, 1..4)
        )
            .prop_map(|(t, ops)| Action::Apply { target: t, ops }),
        // --- install (local path only to avoid network) ---
        name_or_unknown.prop_map(|source| Action::Install {
            source: ProfileSource::IndexEntry(source),
        }),
    ]
}

fn pref_value_to_string(v: &PreferenceValue) -> String {
    match v {
        PreferenceValue::Bool(b) => b.to_string(),
        PreferenceValue::Number(n) => n.to_string(),
        PreferenceValue::Text(s) => s.clone(),
    }
}

fn uuid_like(bytes: &[u8]) -> String {
    let mut h: u64 = 0;
    for b in bytes {
        h = h.wrapping_mul(31).wrapping_add(u64::from(*b));
    }
    format!("{h:x}")
}

fn uuid_like_str(s: &str) -> String {
    uuid_like(s.as_bytes())
}

pub fn action_to_cli(action: &Action, base: &Cli, scratch: &Path) -> Cli {
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
        Action::AddBinding {
            target,
            sub_profile,
            input,
            output,
            modifier,
        } => Commands::AddBinding {
            target: target.clone(),
            sub_profile: *sub_profile,
            input: input.clone(),
            output: output.clone(),
            modifier: modifier.clone(),
        },
        Action::UpdateBinding {
            target,
            sub_profile,
            input,
            output,
            modifier,
        } => Commands::UpdateBinding {
            target: target.clone(),
            sub_profile: *sub_profile,
            input: input.clone(),
            output: output.clone(),
            modifier: modifier.clone(),
        },
        Action::ClearBinding {
            target,
            sub_profile,
            input,
            modifier,
        } => Commands::ClearBinding {
            target: target.clone(),
            sub_profile: *sub_profile,
            input: input.clone(),
            modifier: modifier.clone(),
        },
        Action::SetPreference { target, key, value } => Commands::SetPreference {
            target: target.clone(),
            key: key.clone(),
            value: pref_value_to_string(value),
        },
        Action::UnsetPreference { target, key } => Commands::UnsetPreference {
            target: target.clone(),
            key: key.clone(),
        },
        Action::SetOverride {
            target,
            sub_profile,
            key,
            value,
        } => Commands::SetOverride {
            target: target.clone(),
            sub_profile: *sub_profile,
            key: key.clone(),
            value: pref_value_to_string(value),
        },
        Action::UnsetOverride {
            target,
            sub_profile,
            key,
        } => Commands::UnsetOverride {
            target: target.clone(),
            sub_profile: *sub_profile,
            key: key.clone(),
        },
        Action::SetTitle { target, title } => Commands::SetTitle {
            target: target.clone(),
            title: title.clone(),
        },
        Action::Pull { name } => Commands::Pull {
            name: name.clone(),
            dest: Some(scratch.join(format!("pull-{}.csv", uuid_like_str(name)))),
        },
        Action::Push { name, bytes } => {
            let mut key = name.as_bytes().to_vec();
            key.extend_from_slice(bytes);
            let path = scratch.join(format!("yokectl-prop-{}.csv", uuid_like(&key)));
            let _ = std::fs::write(&path, bytes);
            Commands::Push {
                src: path,
                name: Some(name.clone()),
                validate: false,
            }
        }
        Action::Copy { from, to } => Commands::Copy {
            from: from.clone(),
            to: to.clone(),
        },
        Action::Rename { from, to } => Commands::Rename {
            from: from.clone(),
            to: to.clone(),
        },
        Action::Delete { name, force } => Commands::Delete {
            name: name.clone(),
            force: *force,
        },
        Action::AddSubProfile {
            target,
            name,
            mode,
            channel,
            sub_mode,
        } => Commands::Subprofile {
            cmd: yokectl::cli::SubprofileCmd::Add {
                target: target.clone(),
                name: name.clone(),
                mode: mode.canonical_csv(),
                channel: channel.canonical_csv().to_string(),
                sub_mode: sub_mode.clone(),
            },
        },
        Action::DeleteSubProfile { target, index } => Commands::Subprofile {
            cmd: yokectl::cli::SubprofileCmd::Delete {
                target: target.clone(),
                index: *index,
            },
        },
        Action::RenameSubProfile { target, index, to } => Commands::Subprofile {
            cmd: yokectl::cli::SubprofileCmd::Rename {
                target: target.clone(),
                index: *index,
                to: to.clone(),
            },
        },
        Action::CloneSubProfile { target, index, to } => Commands::Subprofile {
            cmd: yokectl::cli::SubprofileCmd::Clone {
                target: target.clone(),
                index: *index,
                to: to.clone(),
            },
        },
        Action::Apply { target, ops } => {
            let doc = serde_json::json!({ "edits": ops });
            let content = doc.to_string();
            let mut key = target.as_bytes().to_vec();
            key.extend_from_slice(content.as_bytes());
            let hash = uuid_like(&key);
            let path = scratch.join(format!("yokectl-edits-{hash}.json"));
            let _ = std::fs::write(&path, &content);
            Commands::Apply {
                target: target.clone(),
                edits: path,
                dry_run: false,
            }
        }
        Action::Install { source } => {
            // Materialise a temp file for all Install variants so the harness
            // never reaches the network. LocalPath is passed through; IndexEntry
            // and Url are redirected to a known-good local CSV identified by a
            // hash of the source description.
            let src_str = match source {
                ProfileSource::LocalPath(p) => p.to_string_lossy().into_owned(),
                ProfileSource::IndexEntry(n) => n.clone(),
                ProfileSource::Url(u) => u.to_string(),
            };
            let path = if let ProfileSource::LocalPath(p) = source {
                p.clone()
            } else {
                let tmp = scratch.join(format!("yokectl-install-{}.csv", uuid_like_str(&src_str)));
                let _ = std::fs::write(
                    &tmp,
                    b"QuadStick Configuration,Version 1.4,Mock,Install\r\n,,,\r\n*Main,sip_puff,,A\r\n",
                );
                tmp
            };
            Commands::Install {
                source: path.to_string_lossy().into_owned(),
                as_name: None,
                dry_run: true,
                no_validate: true,
                force: false,
            }
        }
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
        Action::CatalogModifiers => Commands::Catalog {
            cmd: yokectl::cli::CatalogCmd::Modifiers,
        },
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
pub mod show_raw;
