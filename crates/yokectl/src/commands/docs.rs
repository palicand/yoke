use std::{fmt::Write as _, fs, path::Path};

use anyhow::{Context, Result};
use clap::{Arg, Command, CommandFactory};

use crate::cli::{Cli, DocsFormat};

pub fn run(format: DocsFormat, out: &Path) -> Result<()> {
    fs::create_dir_all(out).with_context(|| format!("creating docs out dir {}", out.display()))?;
    let root = Cli::command();
    match format {
        DocsFormat::Man => emit_man(&root, out),
        DocsFormat::Md => emit_md(&root, out),
    }
}

fn emit_man(root: &Command, out: &Path) -> Result<()> {
    let man_dir = out.join("man");
    fs::create_dir_all(&man_dir)
        .with_context(|| format!("creating man dir {}", man_dir.display()))?;
    let root_name = root.get_name().to_string();
    walk_man(root, &[root_name.as_str()], &man_dir)
}

fn walk_man(cmd: &Command, path: &[&str], out_dir: &Path) -> Result<()> {
    let dashed = path.join("-");
    let spaced = path.join(" ");
    let dest = out_dir.join(format!("{dashed}.1"));
    let title = dashed.to_uppercase();
    let renamed = cmd
        .clone()
        .display_name(dashed)
        .bin_name(spaced)
        .disable_version_flag(true);
    let mut buf = Vec::new();
    clap_mangen::Man::new(renamed)
        .title(title)
        .render(&mut buf)
        .with_context(|| format!("rendering man for {}", path.join(" ")))?;
    fs::write(&dest, buf).with_context(|| format!("writing {}", dest.display()))?;
    for sub in cmd.get_subcommands() {
        if sub.get_name() == "help" {
            continue;
        }
        let mut child = path.to_vec();
        child.push(sub.get_name());
        walk_man(sub, &child, out_dir)?;
    }
    Ok(())
}

fn emit_md(root: &Command, out: &Path) -> Result<()> {
    let md_dir = out.join("markdown");
    fs::create_dir_all(&md_dir)
        .with_context(|| format!("creating markdown dir {}", md_dir.display()))?;
    let dest = md_dir.join("yokectl.md");
    let mut buf = String::new();
    let root_name = root.get_name().to_string();
    walk_md(root, &[root_name.as_str()], &mut buf);
    fs::write(&dest, buf).with_context(|| format!("writing {}", dest.display()))?;
    Ok(())
}

fn walk_md(cmd: &Command, path: &[&str], out: &mut String) {
    let depth = path.len();
    let prefix = "#".repeat(depth);
    let spaced = path.join(" ");
    let dashed = path.join("-");
    let _ = writeln!(out, "{prefix} {spaced}");
    let _ = writeln!(out);
    if let Some(about) = cmd.get_about() {
        let _ = writeln!(out, "{about}");
        let _ = writeln!(out);
    }
    if let Some(long) = cmd.get_long_about() {
        let _ = writeln!(out, "{long}");
        let _ = writeln!(out);
    }
    let mut renamed = cmd.clone().bin_name(spaced.clone()).display_name(dashed);
    let usage_raw = renamed.render_usage().to_string();
    let usage_trimmed = usage_raw.trim();
    let usage_body = usage_trimmed
        .strip_prefix("Usage:")
        .map_or(usage_trimmed, str::trim);
    if !usage_body.is_empty() {
        let _ = writeln!(out, "**Usage:**");
        let _ = writeln!(out);
        let _ = writeln!(out, "```text");
        let _ = writeln!(out, "{usage_body}");
        let _ = writeln!(out, "```");
        let _ = writeln!(out);
    }
    let args: Vec<&Arg> = cmd
        .get_arguments()
        .filter(|a| !a.is_hide_set() && a.get_id() != "help" && a.get_id() != "version")
        .collect();
    if !args.is_empty() {
        let _ = writeln!(out, "**Options:**");
        let _ = writeln!(out);
        for arg in args {
            let _ = writeln!(out, "- {}", render_arg(arg));
        }
        let _ = writeln!(out);
    }
    for sub in cmd.get_subcommands().filter(|s| s.get_name() != "help") {
        let sub_name = sub.get_name().to_string();
        let mut child = path.to_vec();
        child.push(sub_name.as_str());
        walk_md(sub, &child, out);
    }
}

fn render_arg(arg: &Arg) -> String {
    let mut head = String::new();
    if let Some(short) = arg.get_short() {
        head.push('-');
        head.push(short);
    }
    if let Some(long) = arg.get_long() {
        if !head.is_empty() {
            head.push_str(", ");
        }
        head.push_str("--");
        head.push_str(long);
    }
    if head.is_empty() {
        head = format!("<{}>", arg.get_id().as_str().to_uppercase());
    }
    let help = arg.get_help().map(ToString::to_string).unwrap_or_default();
    if help.is_empty() {
        format!("`{head}`")
    } else {
        format!("`{head}` — {help}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Arg, Command};

    fn fixture() -> Command {
        Command::new("yokectl-fixture")
            .about("fixture root")
            .subcommand(
                Command::new("alpha")
                    .about("alpha command")
                    .arg(Arg::new("name").help("alpha name")),
            )
            .subcommand(
                Command::new("group").about("group parent").subcommand(
                    Command::new("inner")
                        .about("inner leaf")
                        .arg(Arg::new("flag").long("flag").help("inner flag")),
                ),
            )
    }

    #[test]
    fn markdown_nests_three_levels() {
        let cmd = fixture();
        let mut buf = String::new();
        walk_md(&cmd, &["yokectl-fixture"], &mut buf);
        assert!(buf.starts_with("# yokectl-fixture"));
        assert!(buf.contains("\n## yokectl-fixture alpha\n"));
        assert!(buf.contains("\n## yokectl-fixture group\n"));
        assert!(buf.contains("\n### yokectl-fixture group inner\n"));
    }

    #[test]
    fn markdown_emits_options_block_for_args() {
        let cmd = fixture();
        let mut buf = String::new();
        walk_md(&cmd, &["yokectl-fixture"], &mut buf);
        assert!(buf.contains("`--flag` — inner flag"));
        assert!(buf.contains("`<NAME>` — alpha name"));
    }

    #[test]
    fn markdown_skips_help_subcommand() {
        let cmd = fixture();
        let mut buf = String::new();
        walk_md(&cmd, &["yokectl-fixture"], &mut buf);
        assert!(!buf.contains("## help"));
    }

    #[test]
    fn markdown_disambiguates_shared_leaf_names() {
        let cmd = Command::new("yokectl-fixture")
            .subcommand(Command::new("alpha").subcommand(Command::new("list")))
            .subcommand(Command::new("beta").subcommand(Command::new("list")));
        let mut buf = String::new();
        walk_md(&cmd, &["yokectl-fixture"], &mut buf);
        assert!(
            buf.contains("\n### yokectl-fixture alpha list\n"),
            "missing alpha-scoped list heading"
        );
        assert!(
            buf.contains("\n### yokectl-fixture beta list\n"),
            "missing beta-scoped list heading"
        );
    }
}
