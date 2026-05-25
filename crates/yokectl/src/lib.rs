pub mod backend;
pub mod cli;
pub mod commands;
pub mod error;
pub mod output;
pub mod runtime;
pub mod target;

use clap::Parser;
use cli::{CatalogCmd, Commands, IndexCmd, SubprofileCmd};

/// Process-level entry point shared by the binary and any integration harness
/// that wants to drive the CLI in-process. Calls `std::process::exit`.
pub fn entry() -> ! {
    let cli = cli::Cli::parse();
    if cli.no_color {
        console::set_colors_enabled(false);
        console::set_colors_enabled_stderr(false);
    }
    let out = output::Output::from_flags(cli.json);
    init_tracing(cli.verbose);
    let code = match run(cli, &out) {
        Ok(()) => 0,
        Err(err) => {
            let info = error::classify(&err);
            out.emit_error(&info.envelope_code, &err.to_string(), info.details);
            info.code
        }
    };
    std::process::exit(code);
}

#[allow(clippy::too_many_lines)]
pub fn run(cli: cli::Cli, out: &output::Output) -> anyhow::Result<()> {
    let cli::Cli {
        fake_volume,
        command,
        ..
    } = cli;
    match command {
        Commands::Completions { shell } => commands::completions::run(shell),
        Commands::Docs {
            format,
            out: out_dir,
        } => commands::docs::run(format, &out_dir),
        Commands::Manual { topic } => commands::manual::run(out, topic.as_deref()),
        Commands::Topic { name } => commands::topic::run(out, name.as_deref()),
        Commands::Index { cmd } => match cmd {
            IndexCmd::List { refresh } => commands::index::run_list(out, refresh),
            IndexCmd::Search { query } => commands::index::run_search(out, &query),
            IndexCmd::Show { name } => commands::index::run_show(out, &name),
            IndexCmd::Update => commands::index::run_update(out),
            IndexCmd::Browse => commands::index::run_browse(out),
        },
        Commands::Catalog { cmd } => match cmd {
            CatalogCmd::Inputs => commands::catalog::run_inputs(out),
            CatalogCmd::Outputs => commands::catalog::run_outputs(out),
            CatalogCmd::Preferences => commands::catalog::run_preferences(out),
            CatalogCmd::Modes => commands::catalog::run_modes(out),
            CatalogCmd::Channels => commands::catalog::run_channels(out),
        },
        other => run_with_volume(out, &backend::open(fake_volume)?, other),
    }
}

#[allow(clippy::too_many_lines)]
fn run_with_volume(
    out: &output::Output,
    provider: &std::sync::Arc<dyn yoke_volume::VolumeProvider>,
    command: Commands,
) -> anyhow::Result<()> {
    match command {
        Commands::Device => commands::device::run_device(provider, out),
        Commands::Debug => commands::device::run_debug(provider, out),
        Commands::List => commands::profile::run_list(provider, out),
        Commands::Show { target, raw } => commands::profile::run_show(provider, out, &target, raw),
        Commands::Validate { target } => commands::profile::run_validate(provider, out, &target),
        Commands::Pull { name, dest } => commands::profile::run_pull(provider, out, &name, dest),
        Commands::Push {
            src,
            name,
            validate,
        } => commands::profile::run_push(provider, out, &src, name.as_deref(), validate),
        Commands::Copy { from, to } => commands::profile::run_copy(provider, out, &from, &to),
        Commands::Rename { from, to } => commands::profile::run_rename(provider, out, &from, &to),
        Commands::Delete { name, force } => {
            commands::profile::run_delete(provider, out, &name, force)
        }
        Commands::SetTitle { target, title } => {
            commands::edit::run_set_title(provider, out, &target, &title)
        }
        Commands::SetPreference { target, key, value } => {
            commands::edit::run_set_preference(provider, out, &target, &key, &value)
        }
        Commands::UnsetPreference { target, key } => {
            commands::edit::run_unset_preference(provider, out, &target, &key)
        }
        Commands::SetOverride {
            target,
            sub_profile,
            key,
            value,
        } => commands::edit::run_set_override(provider, out, &target, &sub_profile, &key, &value),
        Commands::UnsetOverride {
            target,
            sub_profile,
            key,
        } => commands::edit::run_unset_override(provider, out, &target, &sub_profile, &key),
        Commands::SetBinding {
            target,
            sub_profile,
            input,
            output: output_s,
        } => {
            commands::edit::run_set_binding(provider, out, &target, &sub_profile, &input, &output_s)
        }
        Commands::ClearBinding {
            target,
            sub_profile,
            input,
        } => commands::edit::run_clear_binding(provider, out, &target, &sub_profile, &input),
        Commands::Apply {
            target,
            edits,
            dry_run,
        } => commands::apply::run(provider, out, &target, &edits, dry_run),
        Commands::Bindings {
            target,
            sub_profile,
        } => commands::view::run_bindings(provider, out, &target, sub_profile.as_deref()),
        Commands::Preferences {
            target,
            sub_profile,
            raw,
        } => commands::view::run_preferences(provider, out, &target, sub_profile.as_deref(), raw),
        Commands::Install {
            source,
            as_name,
            dry_run,
            no_validate,
            force,
        } => commands::install::run(
            provider,
            out,
            &source,
            as_name.as_deref(),
            dry_run,
            no_validate,
            force,
        ),
        Commands::Watch => commands::watch::run(provider, out),
        Commands::Subprofile { cmd } => match cmd {
            SubprofileCmd::Add {
                target,
                name,
                mode,
                channel,
                sub_mode,
            } => commands::subprofile::run_add(
                provider,
                out,
                &target,
                &name,
                &mode,
                &channel,
                sub_mode.as_deref(),
            ),
            SubprofileCmd::Delete { target, name } => {
                commands::subprofile::run_delete(provider, out, &target, &name)
            }
            SubprofileCmd::Rename { target, from, to } => {
                commands::subprofile::run_rename(provider, out, &target, &from, &to)
            }
            SubprofileCmd::Clone { target, from, to } => {
                commands::subprofile::run_clone(provider, out, &target, &from, &to)
            }
        },
        Commands::Completions { .. }
        | Commands::Docs { .. }
        | Commands::Manual { .. }
        | Commands::Topic { .. }
        | Commands::Index { .. }
        | Commands::Catalog { .. } => unreachable!("handled before backend open"),
    }
}

fn init_tracing(verbose: u8) {
    let level = match verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}
