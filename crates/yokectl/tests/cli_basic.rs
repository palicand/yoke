use assert_cmd::Command;
use tempfile::tempdir;

fn yokectl() -> Command {
    Command::cargo_bin("yokectl").unwrap()
}

#[test]
fn device_human_prints_present_state() {
    let dir = tempdir().unwrap();
    yokectl()
        .args(["--fake-volume", dir.path().to_str().unwrap(), "device"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Present"));
}

#[test]
fn device_json_emits_state_object() {
    let dir = tempdir().unwrap();
    let out = yokectl()
        .args([
            "--fake-volume",
            dir.path().to_str().unwrap(),
            "--json",
            "device",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["state"]["kind"], "Present");
}

fn seed_profile(dir: &std::path::Path, name: &str, body: &str) {
    std::fs::write(dir.join(name), body).unwrap();
}

const FIXTURE: &str =
    "QuadStick Configuration,Version 1.4,Mock,Default,,\n,,,,\n*Main,sip_puff,,A,inputs\n";

#[test]
fn list_shows_seeded_profile() {
    let dir = tempdir().unwrap();
    seed_profile(dir.path(), "default.csv", FIXTURE);
    yokectl()
        .args(["--fake-volume", dir.path().to_str().unwrap(), "list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("default"));
}

#[test]
fn show_human_prints_title_and_sub_profiles() {
    let dir = tempdir().unwrap();
    seed_profile(dir.path(), "default.csv", FIXTURE);
    yokectl()
        .args([
            "--fake-volume",
            dir.path().to_str().unwrap(),
            "show",
            "default",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("Default"));
}

#[test]
fn show_raw_emits_bytes() {
    let dir = tempdir().unwrap();
    seed_profile(dir.path(), "default.csv", FIXTURE);
    let out = yokectl()
        .args([
            "--fake-volume",
            dir.path().to_str().unwrap(),
            "show",
            "default",
            "--raw",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(out, FIXTURE.as_bytes());
}

#[test]
fn pull_writes_local_file() {
    let dir = tempdir().unwrap();
    seed_profile(dir.path(), "default.csv", FIXTURE);
    let dest = dir.path().join("out.csv");
    yokectl()
        .args([
            "--fake-volume",
            dir.path().to_str().unwrap(),
            "pull",
            "default",
            dest.to_str().unwrap(),
        ])
        .assert()
        .success();
    assert_eq!(std::fs::read(&dest).unwrap(), FIXTURE.as_bytes());
}

#[test]
fn push_then_read_round_trip() {
    let dir = tempdir().unwrap();
    let src = dir.path().join("src.csv");
    std::fs::write(&src, FIXTURE).unwrap();
    yokectl()
        .args([
            "--fake-volume",
            dir.path().to_str().unwrap(),
            "push",
            src.to_str().unwrap(),
            "new",
        ])
        .assert()
        .success();
    assert_eq!(
        std::fs::read(dir.path().join("new.csv")).unwrap(),
        FIXTURE.as_bytes()
    );
}

#[test]
fn copy_then_rename_then_delete() {
    let dir = tempdir().unwrap();
    seed_profile(dir.path(), "default.csv", FIXTURE);
    yokectl()
        .args([
            "--fake-volume",
            dir.path().to_str().unwrap(),
            "copy",
            "default",
            "alt",
        ])
        .assert()
        .success();
    assert!(dir.path().join("alt.csv").exists());
    yokectl()
        .args([
            "--fake-volume",
            dir.path().to_str().unwrap(),
            "rename",
            "alt",
            "alt2",
        ])
        .assert()
        .success();
    assert!(dir.path().join("alt2.csv").exists());
    yokectl()
        .args([
            "--fake-volume",
            dir.path().to_str().unwrap(),
            "delete",
            "alt2",
            "--force",
        ])
        .assert()
        .success();
    assert!(!dir.path().join("alt2.csv").exists());
}

#[test]
fn validate_passes_on_valid_profile() {
    let dir = tempdir().unwrap();
    seed_profile(dir.path(), "default.csv", FIXTURE);
    yokectl()
        .args([
            "--fake-volume",
            dir.path().to_str().unwrap(),
            "validate",
            "default",
        ])
        .assert()
        .success();
}

#[test]
fn debug_emits_sections() {
    let dir = tempdir().unwrap();
    let out = yokectl()
        .args([
            "--fake-volume",
            dir.path().to_str().unwrap(),
            "--json",
            "debug",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert!(v.get("device").is_some());
    assert!(v.get("mount").is_some());
}
