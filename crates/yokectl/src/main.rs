mod backend;
mod cli;
mod commands;
mod error;
mod output;
mod runtime;
mod target;

use clap::Parser;
use cli::Commands;

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

#[allow(clippy::needless_pass_by_value)]
fn run(cli: cli::Cli, out: &output::Output) -> anyhow::Result<()> {
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
        _ => anyhow::bail!("not implemented yet"),
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
