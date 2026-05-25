use clap_complete::CompletionCandidate;
use std::ffi::OsString;

#[derive(Clone)]
pub struct SubProfileNameCompleter;

impl clap_complete::engine::ValueCandidates for SubProfileNameCompleter {
    fn candidates(&self) -> Vec<CompletionCandidate> {
        let argv: Vec<OsString> = std::env::args_os().collect();
        names_from_argv(&argv)
            .into_iter()
            .map(CompletionCandidate::new)
            .collect()
    }
}

/// Reads the profile identified by the first positional target in argv and
/// returns the names of all sub-profiles within it.
fn names_from_argv(argv: &[OsString]) -> Vec<String> {
    let provider = super::resolve_backend_for_completion(argv);
    let Some(target_str) = first_positional_target(argv) else {
        return Vec::new();
    };
    let bytes = match crate::target::Target::classify(&target_str) {
        crate::target::Target::VolumeName(n) => provider
            .and_then(|p| p.read_profile(&n).ok())
            .unwrap_or_default(),
        crate::target::Target::LocalFile(p) => std::fs::read(&p).unwrap_or_default(),
        crate::target::Target::Stdin => Vec::new(),
    };
    if bytes.is_empty() {
        return Vec::new();
    }
    yoke_config::parse(&bytes)
        .ok()
        .map(|parsed| {
            parsed
                .model
                .sub_profiles
                .into_iter()
                .map(|sp| sp.header.profile_name)
                .collect()
        })
        .unwrap_or_default()
}

// Skips the binary name, global flags, and the subcommand keyword; returns the next non-flag token.
fn first_positional_target(argv: &[OsString]) -> Option<String> {
    let mut it = argv.iter().skip(1);
    let mut seen_subcommand = false;
    while let Some(arg) = it.next() {
        let s = arg.to_string_lossy();
        if s == "--fake-volume" {
            it.next();
            continue;
        }
        if s.starts_with("--fake-volume=")
            || s == "--json"
            || s == "--no-color"
            || s.starts_with('-')
        {
            continue;
        }
        if !seen_subcommand {
            seen_subcommand = true;
            continue;
        }
        return Some(s.into_owned());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_positional_skips_global_flags_and_subcommand() {
        let argv = [
            OsString::from("yokectl"),
            OsString::from("--fake-volume"),
            OsString::from("/tmp"),
            OsString::from("--json"),
            OsString::from("set-binding"),
            OsString::from("default"),
            OsString::from("Main"),
        ];
        assert_eq!(first_positional_target(&argv), Some("default".to_string()));
    }
}
