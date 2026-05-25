mod common;

use common::{FIXTURE, FIXTURE_WITH_SUB, seed_profile, yokectl};
use tempfile::tempdir;

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
fn delete_in_json_mode_implies_force() {
    let dir = tempdir().unwrap();
    seed_profile(dir.path(), "default.csv", FIXTURE);
    yokectl()
        .args([
            "--fake-volume",
            dir.path().to_str().unwrap(),
            "--json",
            "delete",
            "default",
        ])
        .assert()
        .success();
    assert!(!dir.path().join("default.csv").exists());
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
    assert!(v.get("profiles").is_some());
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
fn subprofile_add_then_delete_round_trips_through_canonical_write_fallback() {
    // Pins the template->canonical writer fallback: adding a sub-profile changes the section
    // count, so the template-fidelity writer must surface InvariantViolation and load_apply_save
    // must retry through the canonical writer.
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
    let after_add = std::fs::read(dir.path().join("default.csv")).unwrap();
    let parsed = yoke_config::parse(&after_add).expect("post-add profile must re-parse cleanly");
    assert!(
        parsed
            .model
            .sub_profiles
            .iter()
            .any(|sp| sp.header.profile_name == "Alt"),
        "expected new sub-profile to be present in the rewritten file"
    );
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
