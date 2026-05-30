use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::engine::ArgValueCandidates;

use crate::completion::catalog::{CatalogKind, CatalogValueCompleter};
use crate::completion::index::IndexEntryCompleter;
use crate::completion::profile::ProfileNameCompleter;
use crate::completion::subprofile::SubProfileNameCompleter;

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum DocsFormat {
    Man,
    Md,
}

#[derive(Parser, Debug, Clone)]
#[command(name = "yokectl", version, about)]
pub struct Cli {
    #[arg(
        long,
        global = true,
        help = "Use a directory as a fake volume backend instead of the platform device"
    )]
    pub fake_volume: Option<PathBuf>,
    #[arg(
        long,
        global = true,
        help = "Emit machine-readable JSON on stdout (NDJSON for watch)"
    )]
    pub json: bool,
    #[arg(
        short,
        long,
        global = true,
        action = clap::ArgAction::Count,
        help = "Increase tracing verbosity on stderr; repeat (-vv reveals info, -vvv debug)"
    )]
    pub verbose: u8,
    #[arg(
        long,
        global = true,
        help = "Disable ANSI styling (NO_COLOR=1 or non-TTY stdout also disable)"
    )]
    pub no_color: bool,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    #[command(about = "Print the current MountState: VID/PID, location-ID, mode hint, mount point")]
    Device,
    #[command(about = "Emit a rich diagnostic snapshot of device, volume, and parser state")]
    Debug,
    #[command(
        about = "Stream MountState transitions and MountEvents as they arrive",
        long_about = "Stream MountState transitions and MountEvents as they arrive. \
                      Emits NDJSON under --json."
    )]
    Watch,
    #[command(about = "List profiles on the volume with name, kind, size, and modified time")]
    List,
    #[command(about = "Pretty-print the parsed structure of a profile")]
    Show {
        #[arg(help = "Profile name, file path, or '-' for stdin", add = ArgValueCandidates::new(ProfileNameCompleter))]
        target: String,
        #[arg(long, help = "Skip parsing and emit the bytes verbatim")]
        raw: bool,
    },
    #[command(about = "Parse a profile and emit warnings and errors")]
    Validate {
        #[arg(help = "Profile name, file path, or '-' for stdin", add = ArgValueCandidates::new(ProfileNameCompleter))]
        target: String,
    },
    #[command(about = "List preferences on a profile (effective view by default)")]
    Preferences {
        #[arg(help = "Profile name, file path, or '-' for stdin", add = ArgValueCandidates::new(ProfileNameCompleter))]
        target: String,
        #[arg(long, help = "Restrict output to a single sub-profile by name", add = ArgValueCandidates::new(SubProfileNameCompleter))]
        sub_profile: Option<String>,
        #[arg(
            long,
            help = "Show layered view: top-level + per-sub-profile overrides, no resolution"
        )]
        raw: bool,
    },
    #[command(about = "Copy a volume-backed profile to a local path")]
    Pull {
        #[arg(help = "Profile name on the volume", add = ArgValueCandidates::new(ProfileNameCompleter))]
        name: String,
        #[arg(help = "Destination path (defaults to ./<name>.csv)")]
        dest: Option<PathBuf>,
    },
    #[command(about = "Copy a local file to the volume")]
    Push {
        #[arg(help = "Source file path")]
        src: PathBuf,
        #[arg(help = "Destination name on the volume (defaults to source stem)")]
        name: Option<String>,
        #[arg(long, help = "Parse and validate before writing")]
        validate: bool,
    },
    #[command(about = "Copy a profile in place on the volume")]
    Copy {
        #[arg(help = "Source profile name on the volume", add = ArgValueCandidates::new(ProfileNameCompleter))]
        from: String,
        #[arg(help = "Destination profile name on the volume")]
        to: String,
    },
    #[command(about = "Rename a profile in place on the volume")]
    Rename {
        #[arg(help = "Existing profile name on the volume", add = ArgValueCandidates::new(ProfileNameCompleter))]
        from: String,
        #[arg(help = "New profile name on the volume")]
        to: String,
    },
    #[command(
        about = "Delete a profile from the volume",
        long_about = "Delete a profile from the volume. --force skips the confirmation \
                      prompt; --json implies --force because there is no interactive \
                      prompt path under machine-readable output."
    )]
    Delete {
        #[arg(help = "Profile name on the volume", add = ArgValueCandidates::new(ProfileNameCompleter))]
        name: String,
        #[arg(long, help = "Skip the confirmation prompt")]
        force: bool,
    },
    #[command(about = "Set a profile's top-line title")]
    SetTitle {
        #[arg(help = "Profile name, file path, or '-' for stdin", add = ArgValueCandidates::new(ProfileNameCompleter))]
        target: String,
        #[arg(help = "New title")]
        title: String,
    },
    #[command(about = "Set a top-level preference (type inferred from the catalog)")]
    SetPreference {
        #[arg(help = "Profile name, file path, or '-' for stdin", add = ArgValueCandidates::new(ProfileNameCompleter))]
        target: String,
        #[arg(help = "Preference key (validated against catalog::preferences)", add = ArgValueCandidates::new(CatalogValueCompleter(CatalogKind::Preference)))]
        key: String,
        #[arg(help = "Preference value")]
        value: String,
    },
    #[command(about = "Remove a top-level preference")]
    UnsetPreference {
        #[arg(help = "Profile name, file path, or '-' for stdin", add = ArgValueCandidates::new(ProfileNameCompleter))]
        target: String,
        #[arg(help = "Preference key", add = ArgValueCandidates::new(CatalogValueCompleter(CatalogKind::Preference)))]
        key: String,
    },
    #[command(about = "Set a per-sub-profile preference override")]
    SetOverride {
        #[arg(help = "Profile name, file path, or '-' for stdin", add = ArgValueCandidates::new(ProfileNameCompleter))]
        target: String,
        #[arg(help = "Sub-profile name", add = ArgValueCandidates::new(SubProfileNameCompleter))]
        sub_profile: String,
        #[arg(help = "Preference key (validated against catalog::preferences)", add = ArgValueCandidates::new(CatalogValueCompleter(CatalogKind::Preference)))]
        key: String,
        #[arg(help = "Preference value")]
        value: String,
    },
    #[command(about = "Remove a per-sub-profile preference override")]
    UnsetOverride {
        #[arg(help = "Profile name, file path, or '-' for stdin", add = ArgValueCandidates::new(ProfileNameCompleter))]
        target: String,
        #[arg(help = "Sub-profile name", add = ArgValueCandidates::new(SubProfileNameCompleter))]
        sub_profile: String,
        #[arg(help = "Preference key", add = ArgValueCandidates::new(CatalogValueCompleter(CatalogKind::Preference)))]
        key: String,
    },
    #[command(about = "Bind an input phrase to an output in a sub-profile")]
    SetBinding {
        #[arg(help = "Profile name, file path, or '-' for stdin", add = ArgValueCandidates::new(ProfileNameCompleter))]
        target: String,
        #[arg(help = "Sub-profile name", add = ArgValueCandidates::new(SubProfileNameCompleter))]
        sub_profile: String,
        #[arg(help = "Input phrase (validated against catalog::inputs)", add = ArgValueCandidates::new(CatalogValueCompleter(CatalogKind::Input)))]
        input: String,
        #[arg(help = "Output name (validated against catalog::outputs)", add = ArgValueCandidates::new(CatalogValueCompleter(CatalogKind::Output)))]
        output: String,
    },
    #[command(about = "Remove a binding from a sub-profile")]
    ClearBinding {
        #[arg(help = "Profile name, file path, or '-' for stdin", add = ArgValueCandidates::new(ProfileNameCompleter))]
        target: String,
        #[arg(help = "Sub-profile name", add = ArgValueCandidates::new(SubProfileNameCompleter))]
        sub_profile: String,
        #[arg(help = "Input phrase to clear", add = ArgValueCandidates::new(CatalogValueCompleter(CatalogKind::Input)))]
        input: String,
    },
    #[command(about = "Set the modifier on an existing binding in a sub-profile")]
    SetModifier {
        #[arg(help = "Profile name, file path, or '-' for stdin", add = ArgValueCandidates::new(ProfileNameCompleter))]
        target: String,
        #[arg(help = "Sub-profile name", add = ArgValueCandidates::new(SubProfileNameCompleter))]
        sub_profile: String,
        #[arg(help = "Input phrase whose binding to modify", add = ArgValueCandidates::new(CatalogValueCompleter(CatalogKind::Input)))]
        input: String,
        #[arg(help = "Modifier phrase, e.g. \"delay_on 250\" (validated against catalog::modifiers)", add = ArgValueCandidates::new(CatalogValueCompleter(CatalogKind::Modifier)))]
        modifier: String,
    },
    #[command(about = "Manage sub-profiles (add, delete, rename, clone)")]
    Subprofile {
        #[command(subcommand)]
        cmd: SubprofileCmd,
    },
    #[command(about = "Apply a batch of edit operations atomically")]
    Apply {
        #[arg(help = "Profile name, file path, or '-' for stdin", add = ArgValueCandidates::new(ProfileNameCompleter))]
        target: String,
        #[arg(
            long,
            help = "Path to a JSON document {\"edits\": [...]}; '-' reads from stdin"
        )]
        edits: PathBuf,
        #[arg(long, help = "Validate without writing")]
        dry_run: bool,
    },
    #[command(about = "List bindings on a profile, grouped by sub-profile")]
    Bindings {
        #[arg(help = "Profile name, file path, or '-' for stdin", add = ArgValueCandidates::new(ProfileNameCompleter))]
        target: String,
        #[arg(long, help = "Restrict output to a single sub-profile by name", add = ArgValueCandidates::new(SubProfileNameCompleter))]
        sub_profile: Option<String>,
    },
    #[command(
        about = "Install a profile from a path, URL, or community-index name",
        long_about = "Install a profile from a local file path, an HTTP(S) URL, or a bare \
                      community-index name. The source is auto-classified: an existing local \
                      file becomes a path source; a parseable http(s) URL becomes a URL \
                      source (Google Sheets URLs are rewritten to their CSV-export form); \
                      anything else is resolved through the community index. Parse and \
                      validate gate the write by default; --no-validate is an escape hatch."
    )]
    Install {
        #[arg(help = "Local path, URL, or community-index name", add = ArgValueCandidates::new(IndexEntryCompleter))]
        source: String,
        #[arg(
            long = "as",
            help = "Destination filename on the volume (overrides default)"
        )]
        as_name: Option<String>,
        #[arg(long, help = "Print the destination without writing")]
        dry_run: bool,
        #[arg(long, help = "Skip parse and validate (escape hatch; warns on stderr)")]
        no_validate: bool,
        #[arg(long)]
        force: bool,
    },
    #[command(about = "Query the community profile index")]
    Index {
        #[command(subcommand)]
        cmd: IndexCmd,
    },
    #[command(about = "Enumerate catalog values (inputs, outputs, preferences, modes, channels)")]
    Catalog {
        #[command(subcommand)]
        cmd: CatalogCmd,
    },
    #[command(about = "Print a shell-completion script to stdout")]
    Completions {
        #[arg(help = "Target shell (bash, zsh, fish, powershell, elvish)")]
        shell: clap_complete::Shell,
    },
    #[command(
        about = "Generate man pages or a markdown reference under <DIR>",
        long_about = "Generate documentation artifacts derived from the live clap tree. \
                      --format man writes one roff(7) page per command node into <DIR>/man/. \
                      --format md writes a single hierarchical markdown reference into \
                      <DIR>/markdown/yokectl.md. Re-runs overwrite in place."
    )]
    Docs {
        #[arg(long, value_enum, help = "Output format")]
        format: DocsFormat,
        #[arg(long, help = "Output directory (created if missing)")]
        out: PathBuf,
    },
    #[command(
        about = "Open the upstream QuadStick user manual in the default browser",
        long_about = "Open the upstream QuadStick user manual in the default browser. \
                      Bare invocation opens the root page; a known topic slug opens the \
                      matching sub-page. Under --json the resolved URL is printed to \
                      stdout and no browser is launched."
    )]
    Manual {
        #[arg(help = "Optional topic slug; omit to open the manual root")]
        topic: Option<String>,
    },
    #[command(
        about = "Show in-binary topic pages about configuration concepts",
        long_about = "Show in-binary topic pages about configuration concepts (binding \
                      model, sub-profiles, sip-puff thresholds, preferences, install \
                      sources). Bare invocation lists available topics; a slug emits the \
                      topic body as markdown. Under --json the result is wrapped in a \
                      JSON envelope instead."
    )]
    Topic {
        #[arg(help = "Topic slug; omit to list available topics")]
        name: Option<String>,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum SubprofileCmd {
    #[command(about = "Add a sub-profile")]
    Add {
        #[arg(help = "Profile name, file path, or '-' for stdin", add = ArgValueCandidates::new(ProfileNameCompleter))]
        target: String,
        #[arg(help = "Sub-profile name")]
        name: String,
        #[arg(long, help = "SubProfileMode value (validated against catalog::modes)", add = ArgValueCandidates::new(CatalogValueCompleter(CatalogKind::Mode)))]
        mode: String,
        #[arg(long, help = "Channel value (validated against catalog::channels)", add = ArgValueCandidates::new(CatalogValueCompleter(CatalogKind::Channel)))]
        channel: String,
        #[arg(long, help = "Optional sub-mode label")]
        sub_mode: Option<String>,
    },
    #[command(about = "Delete a sub-profile (errors on the last remaining one)")]
    Delete {
        #[arg(help = "Profile name, file path, or '-' for stdin", add = ArgValueCandidates::new(ProfileNameCompleter))]
        target: String,
        #[arg(help = "Sub-profile name", add = ArgValueCandidates::new(SubProfileNameCompleter))]
        name: String,
    },
    #[command(about = "Rename a sub-profile (header only)")]
    Rename {
        #[arg(help = "Profile name, file path, or '-' for stdin", add = ArgValueCandidates::new(ProfileNameCompleter))]
        target: String,
        #[arg(help = "Existing sub-profile name", add = ArgValueCandidates::new(SubProfileNameCompleter))]
        from: String,
        #[arg(help = "New sub-profile name")]
        to: String,
    },
    #[command(about = "Duplicate a sub-profile under a new name")]
    Clone {
        #[arg(help = "Profile name, file path, or '-' for stdin", add = ArgValueCandidates::new(ProfileNameCompleter))]
        target: String,
        #[arg(help = "Source sub-profile name", add = ArgValueCandidates::new(SubProfileNameCompleter))]
        from: String,
        #[arg(help = "Destination sub-profile name")]
        to: String,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum IndexCmd {
    #[command(about = "List the cached community index as a table")]
    List {
        #[arg(long, help = "Force a fetch before listing")]
        refresh: bool,
    },
    #[command(about = "Fuzzy-filter the index by name")]
    Search {
        #[arg(help = "Substring to match against index entry names")]
        query: String,
    },
    #[command(about = "Print an index row, including the resolved CSV URL")]
    Show {
        #[arg(help = "Index entry name", add = ArgValueCandidates::new(IndexEntryCompleter))]
        name: String,
    },
    #[command(about = "Force-refresh the cached community index")]
    Update,
    #[command(about = "Open the community index sheet in the default browser")]
    Browse,
}

#[derive(Subcommand, Debug, Clone)]
pub enum CatalogCmd {
    #[command(about = "List valid input phrases")]
    Inputs,
    #[command(about = "List valid output names")]
    Outputs,
    #[command(about = "List preference keys with their declared value types")]
    Preferences,
    #[command(about = "List SubProfileMode values")]
    Modes,
    #[command(about = "List Channel values")]
    Channels,
}
