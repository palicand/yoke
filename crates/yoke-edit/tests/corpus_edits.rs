//! `YOKE_CORPUS_DIR` breadth: for every real profile, an index-addressed edit must
//! change only the targeted layer. Skipped when the corpus env var is unset.

use std::path::PathBuf;
use yoke_edit::{EditOp, apply};

fn corpus_dir() -> Option<PathBuf> {
    std::env::var_os("YOKE_CORPUS_DIR").map(PathBuf::from)
}

#[test]
fn index_edits_touch_only_their_layer_across_corpus() {
    let Some(dir) = corpus_dir() else {
        eprintln!("YOKE_CORPUS_DIR not set; skipping corpus edit suite.");
        return;
    };
    let mut tested = 0usize;
    for entry in std::fs::read_dir(&dir).expect("read corpus dir") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("csv") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let Ok(parsed) = yoke_config::parse(&bytes) else {
            continue;
        };
        let profile = parsed.model;
        if profile.sub_profiles.len() < 2 {
            continue;
        }
        let before0 = layer_csv(&profile, 0);
        let after = apply(
            profile.clone(),
            &[EditOp::RenameSubProfile {
                index: 1,
                to: "ZZ_probe".into(),
            }],
        )
        .unwrap_or_else(|e| panic!("rename layer 1 of {path:?}: {e}"));
        assert_eq!(
            layer_csv(&after, 0),
            before0,
            "rename of layer 1 changed layer 0 in {path:?}"
        );
        assert_eq!(after.sub_profiles[1].header.profile_name, "ZZ_probe");
        tested += 1;
    }
    eprintln!("Corpus index-edit breadth: {tested} profiles with >=2 sub-profiles tested.");
}

fn layer_csv(p: &yoke_config::model::Profile, idx: usize) -> Vec<u8> {
    let mut one = p.clone();
    one.sub_profiles = vec![p.sub_profiles[idx].clone()];
    yoke_config::write(&one, None).expect("canonical write")
}
