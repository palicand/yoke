mod common;
use common::{FIXTURE_WITH_SUB, seed_profile, yokectl};
use tempfile::tempdir;

// Establish a known binding (lip_soft -> kb_a, normal) via add-binding.
fn seed_and_bind(dir: &std::path::Path) {
    seed_profile(dir, "default.csv", FIXTURE_WITH_SUB);
    yokectl()
        .arg("--fake-volume")
        .arg(dir)
        .args(["add-binding", "default", "0", "lip_soft", "kb_a"])
        .assert()
        .success();
}

#[test]
fn add_binding_with_modifier_writes_csv() {
    let dir = tempdir().unwrap();
    seed_profile(dir.path(), "default.csv", FIXTURE_WITH_SUB);
    yokectl()
        .arg("--fake-volume")
        .arg(dir.path())
        .args([
            "add-binding",
            "default",
            "0",
            "lip_soft",
            "kb_a",
            "--modifier",
            "delay_on 250",
        ])
        .assert()
        .success();
    let csv = std::fs::read_to_string(dir.path().join("default.csv")).unwrap();
    // The modifier must land on the lip_soft -> kb_a row (CSV order is output,modifier,input),
    // not merely appear somewhere in the file.
    assert!(
        csv.contains("kb_a,delay_on 250,lip_soft"),
        "modifier not on the lip_soft -> kb_a row:\n{csv}"
    );
}

#[test]
fn add_binding_duplicate_input_modifier_exits_5() {
    let dir = tempdir().unwrap();
    seed_and_bind(dir.path()); // lip_soft -> kb_a [normal]
    // A second binding for the same (input, modifier=normal) is a conflict.
    yokectl()
        .arg("--fake-volume")
        .arg(dir.path())
        .args(["add-binding", "default", "0", "lip_soft", "kb_b"])
        .assert()
        .code(5);
}

#[test]
fn update_binding_changes_modifier_writes_csv() {
    let dir = tempdir().unwrap();
    seed_and_bind(dir.path());
    // Anchored on (input, output) = (lip_soft, kb_a): changes the modifier.
    yokectl()
        .arg("--fake-volume")
        .arg(dir.path())
        .args([
            "update-binding",
            "default",
            "0",
            "lip_soft",
            "kb_a",
            "--modifier",
            "delay_on 250",
        ])
        .assert()
        .success();
    let csv = std::fs::read_to_string(dir.path().join("default.csv")).unwrap();
    // The modifier changed in place on the lip_soft -> kb_a row; the old normal row is gone.
    assert!(
        csv.contains("kb_a,delay_on 250,lip_soft"),
        "modifier not updated on the lip_soft -> kb_a row:\n{csv}"
    );
    assert!(
        !csv.contains("kb_a,normal,lip_soft"),
        "old normal row should have been replaced, not duplicated:\n{csv}"
    );
}

#[test]
fn update_binding_unknown_modifier_exits_5() {
    let dir = tempdir().unwrap();
    seed_and_bind(dir.path());
    yokectl()
        .arg("--fake-volume")
        .arg(dir.path())
        .args([
            "update-binding",
            "default",
            "0",
            "lip_soft",
            "kb_a",
            "--modifier",
            "togle",
        ])
        .assert()
        .code(5);
}

#[test]
fn bindings_view_surfaces_modifier() {
    let dir = tempdir().unwrap();
    seed_and_bind(dir.path());
    yokectl()
        .arg("--fake-volume")
        .arg(dir.path())
        .args([
            "update-binding",
            "default",
            "0",
            "lip_soft",
            "kb_a",
            "--modifier",
            "delay_on 250",
        ])
        .assert()
        .success();
    yokectl()
        .arg("--fake-volume")
        .arg(dir.path())
        .args(["bindings", "default", "--sub-profile", "Main"])
        .assert()
        .success()
        .stdout(predicates::str::contains("delay_on 250"));
}

#[test]
fn clear_binding_with_modifier_removes_only_that_row() {
    let dir = tempdir().unwrap();
    seed_and_bind(dir.path()); // lip_soft -> kb_a [normal]
    // Add a second, distinct-modifier binding for the same input.
    yokectl()
        .arg("--fake-volume")
        .arg(dir.path())
        .args([
            "add-binding",
            "default",
            "0",
            "lip_soft",
            "kb_b",
            "--modifier",
            "toggle",
        ])
        .assert()
        .success();
    // Clearing the toggle row leaves the normal one.
    yokectl()
        .arg("--fake-volume")
        .arg(dir.path())
        .args([
            "clear-binding",
            "default",
            "0",
            "lip_soft",
            "--modifier",
            "toggle",
        ])
        .assert()
        .success();
    let csv = std::fs::read_to_string(dir.path().join("default.csv")).unwrap();
    // Only the toggle row (lip_soft -> kb_b) is gone; the normal row (lip_soft -> kb_a) survives.
    assert!(!csv.contains("toggle"), "toggle row not removed:\n{csv}");
    assert!(
        !csv.contains("kb_b"),
        "wrong row removed; kb_b (the toggle row) should be gone:\n{csv}"
    );
    assert!(
        csv.contains("kb_a,normal,lip_soft"),
        "normal lip_soft -> kb_a row wrongly removed:\n{csv}"
    );
}

#[test]
fn catalog_modifiers_lists_keywords() {
    yokectl()
        .args(["catalog", "modifiers"])
        .assert()
        .success()
        .stdout(predicates::str::contains("delay_on"))
        .stdout(predicates::str::contains("toggle"));
}
