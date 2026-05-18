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
