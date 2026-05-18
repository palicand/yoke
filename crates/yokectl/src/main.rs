mod backend;
mod cli;
mod output;
mod target;

use clap::Parser;

fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    let out = output::Output::from_flags(cli.json, cli.no_color);
    init_tracing(cli.verbose);
    let _ = cli.command;
    out.emit_error(
        "not-implemented",
        "command not implemented yet",
        serde_json::json!({}),
    );
    std::process::exit(1);
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
