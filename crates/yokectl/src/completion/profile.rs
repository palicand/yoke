use clap_complete::CompletionCandidate;
use std::ffi::OsString;

#[derive(Clone)]
pub struct ProfileNameCompleter;

impl clap_complete::engine::ValueCandidates for ProfileNameCompleter {
    fn candidates(&self) -> Vec<CompletionCandidate> {
        let argv: Vec<OsString> = std::env::args_os().collect();
        let Some(provider) = super::resolve_backend_for_completion(&argv) else {
            return Vec::new();
        };
        provider
            .list_profiles()
            .ok()
            .into_iter()
            .flatten()
            .map(|e| CompletionCandidate::new(e.name.stem().to_string()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn lists_files_in_fake_volume_dir() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("foo.csv"), b"x").unwrap();
        std::fs::write(dir.path().join("bar.csv"), b"x").unwrap();
        let argv = [OsString::from("--fake-volume"), OsString::from(dir.path())];
        let provider = super::super::resolve_backend_for_completion(&argv).expect("backend");
        let names: Vec<String> = provider
            .list_profiles()
            .unwrap()
            .into_iter()
            .map(|e| e.name.stem().to_string())
            .collect();
        assert!(names.contains(&"foo".to_string()));
        assert!(names.contains(&"bar".to_string()));
    }
}
