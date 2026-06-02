use assert_cmd::Command;
use tempfile::tempdir;

fn yokectl() -> Command {
    Command::cargo_bin("yokectl").unwrap()
}

const FIXTURE_WITH_SUB: &str = "QuadStick Configuration,Version 1.4,Mock,Default\r\n\
Profile Name,Main,Mouse,usb\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mouse_left,normal,left,\r\n\
\r\n";

#[test]
fn subprofile_lifecycle_add_clone_rename_delete() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("default.csv"), FIXTURE_WITH_SUB).unwrap();
    let vol = dir.path().to_str().unwrap();
    yokectl()
        .args([
            "--fake-volume",
            vol,
            "subprofile",
            "add",
            "default",
            "Alt",
            "--mode",
            "Mouse",
            "--channel",
            "usb",
        ])
        .assert()
        .success();
    yokectl()
        .args([
            "--fake-volume",
            vol,
            "subprofile",
            "clone",
            "default",
            "1",
            "Alt2",
        ])
        .assert()
        .success();
    yokectl()
        .args([
            "--fake-volume",
            vol,
            "subprofile",
            "rename",
            "default",
            "2",
            "Renamed",
        ])
        .assert()
        .success();
    // The rename keeps the section count, so it stays on the template-fidelity writer;
    // confirm the new name actually persisted to the file rather than being dropped.
    let bindings = yokectl()
        .args(["--json", "--fake-volume", vol, "bindings", "default"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&bindings).unwrap();
    assert_eq!(v["sub_profiles"][2]["name"], "Renamed");
    yokectl()
        .args(["--fake-volume", vol, "subprofile", "delete", "default", "1"])
        .assert()
        .success();
}

// Round-trips every CSV in YOKE_CORPUS_DIR through install + pull. Gated on the
// env var because we deliberately do not check in the corpus; the test no-ops
// when unset rather than failing CI.
#[test]
fn corpus_round_trip() {
    let Ok(corpus) = std::env::var("YOKE_CORPUS_DIR") else {
        return;
    };
    let dir = tempdir().unwrap();
    let vol = dir.path().to_str().unwrap();
    let mut succeeded = 0_u32;
    for entry in walkdir::WalkDir::new(&corpus).into_iter().flatten() {
        if entry.path().extension().and_then(|s| s.to_str()) != Some("csv") {
            continue;
        }
        let Some(stem) = entry.path().file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        // ProfileName forbids slashes and other punctuation; the corpus has files
        // with arbitrary stems, so skip anything we can't name on the volume.
        let sanitized = stem.replace(['/', ' '], "_").to_lowercase();
        let install = yokectl()
            .args([
                "--fake-volume",
                vol,
                "install",
                entry.path().to_str().unwrap(),
                "--as",
                &sanitized,
                "--no-validate",
            ])
            .output()
            .unwrap();
        if !install.status.success() {
            // Some corpus files are intentionally malformed test artifacts; skip them
            // rather than fail the run.
            continue;
        }
        let pulled = dir.path().join("pulled.csv");
        yokectl()
            .args([
                "--fake-volume",
                vol,
                "pull",
                &sanitized,
                pulled.to_str().unwrap(),
            ])
            .assert()
            .success();
        assert_eq!(
            std::fs::read(&pulled).unwrap(),
            std::fs::read(entry.path()).unwrap(),
            "byte mismatch for {}",
            entry.path().display()
        );
        succeeded += 1;
        let _ = std::fs::remove_file(&pulled);
    }
    assert!(succeeded > 0, "no corpus entries successfully installed");
}
