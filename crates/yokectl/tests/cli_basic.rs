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

const FIXTURE_WITH_SUB: &str = "QuadStick Configuration,Version 1.4,Mock,Default\r\n\
Profile Name,Main,Mouse,usb\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mouse_left,normal,left,\r\n\
\r\n";

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

#[test]
fn set_title_round_trips_through_file() {
    let dir = tempdir().unwrap();
    seed_profile(dir.path(), "default.csv", FIXTURE);
    yokectl()
        .args([
            "--fake-volume",
            dir.path().to_str().unwrap(),
            "set-title",
            "default",
            "Renamed",
        ])
        .assert()
        .success();
    let body = std::fs::read_to_string(dir.path().join("default.csv")).unwrap();
    assert!(body.contains("Renamed"));
}

#[test]
fn set_preference_writes_value() {
    let dir = tempdir().unwrap();
    seed_profile(dir.path(), "default.csv", FIXTURE);
    yokectl()
        .args([
            "--fake-volume",
            dir.path().to_str().unwrap(),
            "set-preference",
            "default",
            "volume",
            "55",
        ])
        .assert()
        .success();
}

#[test]
fn set_binding_with_bad_input_returns_exit_5_and_suggestions() {
    let dir = tempdir().unwrap();
    seed_profile(dir.path(), "default.csv", FIXTURE_WITH_SUB);
    let out = yokectl()
        .args([
            "--fake-volume",
            dir.path().to_str().unwrap(),
            "--json",
            "set-binding",
            "default",
            "Main",
            "lip_sof",
            "kb_a",
        ])
        .assert()
        .code(5)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["error"]["code"], "edit-unknown-input");
    assert!(
        v["error"]["details"]["suggestions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s == "lip_soft")
    );
}

#[test]
fn apply_batch_succeeds_when_valid() {
    let dir = tempdir().unwrap();
    seed_profile(dir.path(), "default.csv", FIXTURE);
    let edits = dir.path().join("edits.json");
    std::fs::write(
        &edits,
        r#"{"edits":[{"op":"set-title","title":"Batched"}]}"#,
    )
    .unwrap();
    yokectl()
        .args([
            "--fake-volume",
            dir.path().to_str().unwrap(),
            "apply",
            "default",
            "--edits",
            edits.to_str().unwrap(),
        ])
        .assert()
        .success();
}

#[test]
fn apply_batch_rejects_and_does_not_modify_file_on_invalid_op() {
    let dir = tempdir().unwrap();
    seed_profile(dir.path(), "default.csv", FIXTURE);
    let snapshot = std::fs::read(dir.path().join("default.csv")).unwrap();
    let edits = dir.path().join("edits.json");
    std::fs::write(
        &edits,
        r#"{"edits":[{"op":"set-title","title":"A"},{"op":"delete-sub-profile","name":"Ghost"}]}"#,
    )
    .unwrap();
    yokectl()
        .args([
            "--fake-volume",
            dir.path().to_str().unwrap(),
            "apply",
            "default",
            "--edits",
            edits.to_str().unwrap(),
        ])
        .assert()
        .code(5);
    assert_eq!(
        std::fs::read(dir.path().join("default.csv")).unwrap(),
        snapshot
    );
}

#[test]
fn catalog_outputs_lists_kb_a() {
    yokectl()
        .args(["catalog", "outputs"])
        .assert()
        .success()
        .stdout(predicates::str::contains("kb_a"));
}

#[test]
fn catalog_channels_lists_usb() {
    yokectl()
        .args(["catalog", "channels"])
        .assert()
        .success()
        .stdout(predicates::str::contains("usb"));
}

#[test]
fn completions_bash_emits_function() {
    yokectl()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicates::str::contains("_yokectl()"));
}

#[test]
fn completions_fish_emits_complete() {
    yokectl()
        .args(["completions", "fish"])
        .assert()
        .success()
        .stdout(predicates::str::contains("complete -c yokectl"));
}

#[test]
fn completions_zsh_emits_compdef() {
    yokectl()
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicates::str::contains("#compdef yokectl"));
}

#[test]
fn completions_powershell_emits_register() {
    yokectl()
        .args(["completions", "powershell"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Register-ArgumentCompleter"));
}

#[test]
fn subprofile_add_then_delete() {
    let dir = tempdir().unwrap();
    seed_profile(dir.path(), "default.csv", FIXTURE_WITH_SUB);
    yokectl()
        .args([
            "--fake-volume",
            dir.path().to_str().unwrap(),
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
            dir.path().to_str().unwrap(),
            "subprofile",
            "delete",
            "default",
            "Alt",
        ])
        .assert()
        .success();
}
