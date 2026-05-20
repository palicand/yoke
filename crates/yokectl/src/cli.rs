use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "yokectl", version, about)]
pub struct Cli {
    #[arg(long, global = true)]
    pub fake_volume: Option<PathBuf>,
    #[arg(long, global = true)]
    pub json: bool,
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,
    #[arg(long, global = true)]
    pub no_color: bool,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Device,
    Debug,
    Watch {
        #[arg(long)]
        include_poll: bool,
    },
    List,
    Show {
        target: String,
        #[arg(long)]
        raw: bool,
    },
    Validate {
        target: String,
    },
    Pull {
        name: String,
        dest: Option<PathBuf>,
    },
    Push {
        src: PathBuf,
        name: Option<String>,
        #[arg(long)]
        validate: bool,
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
        #[arg(long)]
        force: bool,
    },
    SetTitle {
        target: String,
        title: String,
    },
    SetPreference {
        target: String,
        key: String,
        value: String,
    },
    UnsetPreference {
        target: String,
        key: String,
    },
    SetOverride {
        target: String,
        sub_profile: String,
        key: String,
        value: String,
    },
    UnsetOverride {
        target: String,
        sub_profile: String,
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
    Subprofile {
        #[command(subcommand)]
        cmd: SubprofileCmd,
    },
    Apply {
        target: String,
        #[arg(long)]
        edits: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    Install {
        source: String,
        #[arg(long = "as")]
        as_name: Option<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        no_validate: bool,
    },
    Index {
        #[command(subcommand)]
        cmd: IndexCmd,
    },
    Catalog {
        #[command(subcommand)]
        cmd: CatalogCmd,
    },
    Completions {
        shell: clap_complete::Shell,
    },
    Manual {
        topic: Option<String>,
    },
    Topic {
        name: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum SubprofileCmd {
    Add {
        target: String,
        name: String,
        #[arg(long)]
        mode: String,
        #[arg(long)]
        channel: String,
        #[arg(long)]
        sub_mode: Option<String>,
    },
    Delete {
        target: String,
        name: String,
    },
    Rename {
        target: String,
        from: String,
        to: String,
    },
    Clone {
        target: String,
        from: String,
        to: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum IndexCmd {
    List {
        #[arg(long)]
        refresh: bool,
    },
    Search {
        query: String,
    },
    Show {
        name: String,
    },
    Update,
    #[command(about = "Open the community index sheet in the default browser")]
    Browse,
}

#[derive(Subcommand, Debug)]
pub enum CatalogCmd {
    Inputs,
    Outputs,
    Preferences,
    Modes,
    Channels,
}
