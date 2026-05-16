use std::path::PathBuf;

use yoke_config::{parse, write};

#[test]
fn corpus_dir_round_trips() {
    let Some(env_val) = std::env::var_os("YOKE_CORPUS_DIR") else {
        eprintln!("YOKE_CORPUS_DIR not set; skipping corpus round-trip suite.");
        return;
    };
    let dir = PathBuf::from(env_val);
    assert!(dir.is_dir(), "YOKE_CORPUS_DIR={dir:?} is not a directory");

    let mut failures: Vec<(PathBuf, String)> = Vec::new();
    let mut tested = 0usize;

    for entry in walkdir::WalkDir::new(&dir)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("csv") {
            continue;
        }

        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                failures.push((path.to_path_buf(), format!("read: {e}")));
                continue;
            }
        };
        let result = match parse(&bytes) {
            Ok(r) => r,
            Err(e) => {
                failures.push((path.to_path_buf(), format!("parse: {e}")));
                continue;
            }
        };
        match write(&result.model, Some(&result.raw)) {
            Ok(out) => {
                tested += 1;
                if out != bytes {
                    failures.push((
                        path.to_path_buf(),
                        format!(
                            "byte mismatch (input {} bytes, output {} bytes)",
                            bytes.len(),
                            out.len()
                        ),
                    ));
                }
            }
            Err(e) => failures.push((path.to_path_buf(), format!("write: {e}"))),
        }
    }

    eprintln!(
        "Corpus round-trip: {tested} files tested, {} failures.",
        failures.len()
    );
    if !failures.is_empty() {
        for (p, reason) in &failures {
            eprintln!("  {} — {reason}", p.display());
        }
        panic!("{} fixtures failed corpus round-trip", failures.len());
    }
}
