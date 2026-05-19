use anyhow::Result;
use clap::CommandFactory;
use clap_complete::Shell;

#[allow(clippy::unnecessary_wraps)]
pub fn run(shell: Shell) -> Result<()> {
    let mut cmd = crate::cli::Cli::command();
    clap_complete::generate(shell, &mut cmd, "yokectl", &mut std::io::stdout());
    Ok(())
}
