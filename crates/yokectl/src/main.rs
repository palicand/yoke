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
