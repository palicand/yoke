pub mod backend;
pub mod cli;
pub mod error;
pub mod output;
pub mod runtime;
pub mod target;

use clap::Parser;

/// Process-level entry point. Calls `std::process::exit`.
pub fn entry() -> ! {
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
pub fn run(cli: cli::Cli, _out: &output::Output) -> anyhow::Result<()> {
    let _provider = backend::open(cli.fake_volume)?;
    anyhow::bail!("not implemented yet")
}

pub fn init_tracing(verbose: u8) {
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
