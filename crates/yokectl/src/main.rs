mod backend;
mod cli;
mod commands;
mod error;
mod output;
mod runtime;
mod target;

use clap::Parser;
use cli::{CatalogCmd, Commands, IndexCmd, SubprofileCmd};

fn main() {
    let cli = cli::Cli::parse();
    let out = output::Output::from_flags(cli.json, cli.no_color);
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

#[allow(clippy::needless_pass_by_value, clippy::too_many_lines)]
fn run(cli: cli::Cli, out: &output::Output) -> anyhow::Result<()> {
    // Completions doesn't need a mounted volume; resolving the backend here
    // would make `yokectl completions` fail on hosts without a device.
    if let Commands::Completions { shell } = cli.command {
        return commands::completions::run(shell);
    }
    let provider = backend::open(cli.fake_volume.clone())?;
    match cli.command {
        Commands::Device => commands::device::run_device(&provider, out),
        Commands::Debug => commands::device::run_debug(&provider, out),
        Commands::List => commands::profile::run_list(&provider, out),
        Commands::Show { target, raw } => commands::profile::run_show(&provider, out, &target, raw),
        Commands::Validate { target } => commands::profile::run_validate(&provider, out, &target),
        Commands::Pull { name, dest, raw } => {
            commands::profile::run_pull(&provider, out, &name, dest, raw)
        }
        Commands::Push {
            src,
            name,
            validate,
        } => commands::profile::run_push(&provider, out, &src, name.as_deref(), validate),
        Commands::Copy { from, to } => commands::profile::run_copy(&provider, out, &from, &to),
        Commands::Rename { from, to } => commands::profile::run_rename(&provider, out, &from, &to),
        Commands::Delete { name, force } => {
            commands::profile::run_delete(&provider, out, &name, force)
        }
        Commands::SetTitle { target, title } => {
            commands::edit::run_set_title(&provider, out, &target, &title)
        }
        Commands::SetPreference { target, key, value } => {
            commands::edit::run_set_preference(&provider, out, &target, &key, &value)
        }
        Commands::UnsetPreference { target, key } => {
            commands::edit::run_unset_preference(&provider, out, &target, &key)
        }
        Commands::SetOverride {
            target,
            sub_profile,
            key,
            value,
        } => commands::edit::run_set_override(&provider, out, &target, &sub_profile, &key, &value),
        Commands::UnsetOverride {
            target,
            sub_profile,
            key,
        } => commands::edit::run_unset_override(&provider, out, &target, &sub_profile, &key),
        Commands::SetBinding {
            target,
            sub_profile,
            input,
            output: output_s,
        } => commands::edit::run_set_binding(
            &provider,
            out,
            &target,
            &sub_profile,
            &input,
            &output_s,
        ),
        Commands::ClearBinding {
            target,
            sub_profile,
            input,
        } => commands::edit::run_clear_binding(&provider, out, &target, &sub_profile, &input),
        Commands::Apply {
            target,
            edits,
            dry_run,
        } => commands::apply::run(&provider, out, &target, &edits, dry_run),
        Commands::Install {
            source,
            as_name,
            dry_run,
            no_validate,
        } => commands::install::run(
            &provider,
            out,
            &source,
            as_name.as_deref(),
            dry_run,
            no_validate,
        ),
        Commands::Watch { include_poll } => commands::watch::run(&provider, out, include_poll),
        Commands::Index { cmd } => match cmd {
            IndexCmd::List { refresh } => commands::index::run_list(out, refresh),
            IndexCmd::Search { query } => commands::index::run_search(out, &query),
            IndexCmd::Show { name } => commands::index::run_show(out, &name),
            IndexCmd::Update => commands::index::run_update(out),
        },
        Commands::Catalog { cmd } => match cmd {
            CatalogCmd::Inputs => commands::catalog::run_inputs(out),
            CatalogCmd::Outputs => commands::catalog::run_outputs(out),
            CatalogCmd::Preferences => commands::catalog::run_preferences(out),
            CatalogCmd::Modes => commands::catalog::run_modes(out),
            CatalogCmd::Channels => commands::catalog::run_channels(out),
        },
        Commands::Subprofile { cmd } => match cmd {
            SubprofileCmd::Add {
                target,
                name,
                mode,
                channel,
                sub_mode,
            } => commands::subprofile::run_add(
                &provider,
                out,
                &target,
                &name,
                &mode,
                &channel,
                sub_mode.as_deref(),
            ),
            SubprofileCmd::Delete { target, name } => {
                commands::subprofile::run_delete(&provider, out, &target, &name)
            }
            SubprofileCmd::Rename { target, from, to } => {
                commands::subprofile::run_rename(&provider, out, &target, &from, &to)
            }
            SubprofileCmd::Clone { target, from, to } => {
                commands::subprofile::run_clone(&provider, out, &target, &from, &to)
            }
        },
        Commands::Completions { .. } => unreachable!("handled before backend open"),
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
