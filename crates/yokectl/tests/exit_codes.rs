mod common;

use common::{FIXTURE_WITH_SUB, yokectl};
use tempfile::tempdir;

#[test]
fn not_present_exits_3() {
    let dir = tempdir().unwrap();
    let missing = dir.path().join("missing");
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
    // 0xc3 0x28 is a well-formed UTF-8 prefix followed by an invalid continuation byte;
    // unambiguously fails UTF-8 decode (unlike 0xff 0xfe, which is a valid UTF-16 BOM).
    std::fs::write(dir.path().join("bad.csv"), [0xc3, 0x28]).unwrap();
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
