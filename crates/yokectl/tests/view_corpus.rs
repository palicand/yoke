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

// Re-serialize one sub-profile so layers can be compared byte-wise without relying on the
// whole-profile template-fidelity writer (which preserves header rows verbatim).
fn layer_csv(p: &yoke_config::model::Profile, idx: usize) -> Vec<u8> {
    let mut one = p.clone();
    one.sub_profiles = vec![p.sub_profiles[idx].clone()];
    yoke_config::write(&one, None).expect("canonical write")
}

#[test]
fn index_addressed_edit_leaves_sibling_layer_untouched_on_corpus() {
    let Some(corpus) = corpus_dir() else {
        eprintln!("skipping: YOKE_CORPUS_DIR not set");
        return;
    };
    let mut tested = 0usize;
    for entry in WalkDir::new(&corpus).max_depth(2) {
        let entry = entry.unwrap();
        if !entry.path().to_string_lossy().ends_with(".csv") {
            continue;
        }
        // Only multi-layer profiles can demonstrate non-interference between layers.
        let Ok(parsed) = yoke_config::parse(&std::fs::read(entry.path()).unwrap()) else {
            continue;
        };
        if parsed.model.sub_profiles.len() < 2 {
            continue;
        }
        let before0 = layer_csv(&parsed.model, 0);

        let dir = tempdir().unwrap();
        let file_name = entry.path().file_name().unwrap();
        let dest = dir.path().join(file_name);
        std::fs::copy(entry.path(), &dest).unwrap();
        let stem = entry
            .path()
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .into_owned();

        // Index-addressed rename of layer 1 through the CLI mutates the volume file in place.
        yokectl()
            .arg("--fake-volume")
            .arg(dir.path())
            .arg("subprofile")
            .arg("rename")
            .arg(&stem)
            .arg("1")
            .arg("ZZ_probe")
            .assert()
            .success();

        // Re-read the mutated file and confirm layer 0 is byte-identical: an edit aimed at
        // index 1 must not corrupt the sibling layer (the addressing-bug class).
        let after = yoke_config::parse(&std::fs::read(&dest).unwrap())
            .unwrap_or_else(|e| panic!("re-parse after rename of {entry:?}: {e}"))
            .model;
        assert!(
            after.sub_profiles.len() >= 2,
            "layer count shrank after rename for {entry:?}"
        );
        assert_eq!(
            layer_csv(&after, 0),
            before0,
            "index-addressed rename of layer 1 changed layer 0 for {entry:?}"
        );
        tested += 1;
    }
    eprintln!("Corpus index-edit non-interference: {tested} multi-layer profiles tested.");
}
