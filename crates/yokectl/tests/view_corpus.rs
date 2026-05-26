mod common;
use common::yokectl;
use std::path::PathBuf;
use tempfile::tempdir;
use walkdir::WalkDir;

fn corpus_dir() -> Option<PathBuf> {
    std::env::var_os("YOKE_CORPUS_DIR").map(PathBuf::from)
}

#[test]
fn bindings_and_preferences_succeed_on_corpus_csvs() {
    let Some(corpus) = corpus_dir() else {
        eprintln!("skipping: YOKE_CORPUS_DIR not set");
        return;
    };
    for entry in WalkDir::new(&corpus).max_depth(2) {
        let entry = entry.unwrap();
        if !entry.path().to_string_lossy().ends_with(".csv") {
            continue;
        }
        let dir = tempdir().unwrap();
        let dest = dir.path().join(entry.path().file_name().unwrap());
        std::fs::copy(entry.path(), &dest).unwrap();
        let stem = entry
            .path()
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        for sub in ["bindings", "preferences"] {
            let out = yokectl()
                .arg("--json")
                .arg("--fake-volume")
                .arg(dir.path())
                .arg(sub)
                .arg(&stem)
                .assert()
                .success();
            let bytes = out.get_output().stdout.clone();
            let _: serde_json::Value = serde_json::from_slice(&bytes)
                .unwrap_or_else(|e| panic!("invalid JSON from {sub} {entry:?}: {e}"));
        }
    }
}
