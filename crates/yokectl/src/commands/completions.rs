use anyhow::Result;
use clap::CommandFactory;
use clap_complete::Shell;

// Result<()> matches the dispatcher contract; the body is infallible because
// clap_complete::generate panics on a malformed cmd, which is a bug, not a
// runtime error.
#[allow(clippy::unnecessary_wraps)]
pub fn run(shell: Shell) -> Result<()> {
    let mut cmd = crate::cli::Cli::command();
    clap_complete::generate(shell, &mut cmd, "yokectl", &mut std::io::stdout());
    Ok(())
}
