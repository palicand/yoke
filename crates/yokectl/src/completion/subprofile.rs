use clap_complete::CompletionCandidate;
use std::ffi::OsString;

#[derive(Clone)]
pub struct SubProfileIndexCompleter;

impl clap_complete::engine::ValueCandidates for SubProfileIndexCompleter {
    fn candidates(&self) -> Vec<CompletionCandidate> {
        let argv: Vec<OsString> = std::env::args_os().collect();
        index_candidates_from_argv(&argv)
            .into_iter()
            .map(|(idx, label)| CompletionCandidate::new(idx).help(Some(label.into())))
            .collect()
    }
}

/// Reads the profile identified by the first positional target in argv and returns
/// (index, label) pairs for every sub-profile, where label mirrors the GUI/`bindings`
/// display ("Left joy · Normal").
fn index_candidates_from_argv(argv: &[OsString]) -> Vec<(String, String)> {
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
                .iter()
                .enumerate()
                .map(|(i, sp)| (i.to_string(), sub_profile_label(sp)))
                .collect()
        })
        .unwrap_or_default()
}

fn sub_profile_label(sp: &yoke_config::model::SubProfile) -> String {
    let base = {
        let name = sp.header.profile_name.trim();
        if name.is_empty() {
            sp.header.mode.canonical_csv()
        } else {
            name.to_owned()
        }
    };
    let sub = sp.header.sub_mode.trim();
    if sub.is_empty() {
        base
    } else {
        format!("{base} · {sub}")
    }
}

// Skips the binary name, global flags, and the subcommand keyword(s); returns the next non-flag token.
fn first_positional_target(argv: &[OsString]) -> Option<String> {
    // `clap_complete`'s `CompleteEnv` invokes us as `<bin> -- <bin> <user line...>`,
    // so the real command line begins after the `--` separator and is prefixed by a
    // re-injected binary name we must also drop. Direct/in-process callers pass the
    // bare argv, where only the leading binary name needs skipping.
    let mut it = argv
        .iter()
        .position(|a| a == "--")
        .map_or_else(|| argv.iter().skip(1), |i| argv.iter().skip(i + 2));
    // `subprofile` is the only command with a nested subcommand (`subprofile delete <target>`),
    // so its target sits one keyword further in than a flat command's; skip two keywords there.
    let mut keywords_to_skip = 1usize;
    let mut skipped = 0usize;
    while let Some(arg) = it.next() {
        let s = arg.to_string_lossy();
        if s == "--fake-volume" {
            it.next();
            continue;
        }
        if s.starts_with('-') {
            continue;
        }
        if skipped == 0 && s == "subprofile" {
            keywords_to_skip = 2;
        }
        if skipped < keywords_to_skip {
            skipped += 1;
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

    #[test]
    fn first_positional_skips_nested_subprofile_subcommand() {
        // `subprofile <cmd> <target>` nests one level: the target is the token after the
        // nested subcommand keyword, not the keyword itself.
        let argv = [
            OsString::from("yokectl"),
            OsString::from("subprofile"),
            OsString::from("delete"),
            OsString::from("default"),
        ];
        assert_eq!(first_positional_target(&argv), Some("default".to_string()));
    }

    #[test]
    fn first_positional_handles_complete_env_argv() {
        // The shape clap_complete actually injects: `<bin> -- <bin> <user line...>`.
        let argv = [
            OsString::from("yokectl"),
            OsString::from("--"),
            OsString::from("yokectl"),
            OsString::from("--fake-volume"),
            OsString::from("/tmp"),
            OsString::from("set-binding"),
            OsString::from("default"),
            OsString::from("Main"),
        ];
        assert_eq!(first_positional_target(&argv), Some("default".to_string()));
    }

    #[test]
    fn index_candidates_label_each_sub_profile() {
        // <bin> <subcommand> <target> ... ; target resolves to a local file fixture.
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../yoke-edit/tests/fixtures/default.csv"
        );
        let argv = [
            OsString::from("yokectl"),
            OsString::from("update-binding"),
            OsString::from(path),
        ];
        let cands = index_candidates_from_argv(&argv);
        assert_eq!(cands.len(), 7, "default.csv has 7 sub-profiles");
        assert_eq!(cands[0].0, "0");
        assert!(
            !cands[0].1.is_empty(),
            "each candidate carries a display label"
        );
    }
}
