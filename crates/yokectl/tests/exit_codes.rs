use assert_cmd::Command;
use tempfile::tempdir;

fn yokectl() -> Command {
    Command::cargo_bin("yokectl").unwrap()
}

// Shared with cli_basic.rs but copied to avoid cross-test-file imports.
const FIXTURE_WITH_SUB: &str = "QuadStick Configuration,Version 1.4,Mock,Default\r\n\
Profile Name,Main,Mouse,usb\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mouse_left,normal,left,\r\n\
\r\n";

#[test]
fn not_present_exits_3() {
    let missing = std::env::temp_dir().join("yokectl-exit-codes-nonexistent-zzz");
    let _ = std::fs::remove_dir_all(&missing);
    yokectl()
        .args(["--fake-volume", missing.to_str().unwrap(), "list"])
        .assert()
        .code(3);
}

#[test]
fn invalid_name_exits_2() {
    let dir = tempdir().unwrap();
    yokectl()
        .args([
            "--fake-volume",
            dir.path().to_str().unwrap(),
            "pull",
            "bad/name",
        ])
        .assert()
        .code(2);
}

#[test]
fn edit_unknown_input_exits_5_with_suggestion() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("default.csv"), FIXTURE_WITH_SUB).unwrap();
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
    let suggestions = v["error"]["details"]["suggestions"].as_array().unwrap();
    assert!(suggestions.iter().any(|s| s == "lip_soft"));
}

#[test]
fn parse_error_exits_4() {
    let dir = tempdir().unwrap();
    // Invalid UTF-8 triggers ParseError::Encoding inside yoke_config::parse,
    // which the classifier maps to exit 4.
    std::fs::write(dir.path().join("bad.csv"), [0xff, 0xfe, 0xff, 0xfe]).unwrap();
    yokectl()
        .args([
            "--fake-volume",
            dir.path().to_str().unwrap(),
            "validate",
            "bad",
        ])
        .assert()
        .code(4);
}
