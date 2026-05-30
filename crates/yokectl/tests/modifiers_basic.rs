mod common;
use common::{FIXTURE_WITH_SUB, seed_profile, yokectl};
use tempfile::tempdir;

// Establish a known binding (lip_soft -> kb_a) then set its modifier.
fn seed_and_bind(dir: &std::path::Path) {
    seed_profile(dir, "default.csv", FIXTURE_WITH_SUB);
    yokectl()
        .arg("--fake-volume")
        .arg(dir)
        .args(["set-binding", "default", "Main", "lip_soft", "kb_a"])
        .assert()
        .success();
}

#[test]
fn set_modifier_on_existing_binding_writes_csv() {
    let dir = tempdir().unwrap();
    seed_and_bind(dir.path());
    yokectl()
        .arg("--fake-volume")
        .arg(dir.path())
        .args([
            "set-modifier",
            "default",
            "Main",
            "lip_soft",
            "delay_on 250",
        ])
        .assert()
        .success();
    let csv = std::fs::read_to_string(dir.path().join("default.csv")).unwrap();
    assert!(csv.contains("delay_on 250"), "modifier not written:\n{csv}");
}

#[test]
fn set_modifier_unknown_modifier_exits_5() {
    let dir = tempdir().unwrap();
    seed_and_bind(dir.path());
    yokectl()
        .arg("--fake-volume")
        .arg(dir.path())
        .args(["set-modifier", "default", "Main", "lip_soft", "togle"])
        .assert()
        .code(5);
}

#[test]
fn bindings_view_surfaces_set_modifier() {
    let dir = tempdir().unwrap();
    seed_and_bind(dir.path());
    yokectl()
        .arg("--fake-volume")
        .arg(dir.path())
        .args([
            "set-modifier",
            "default",
            "Main",
            "lip_soft",
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
fn catalog_modifiers_lists_keywords() {
    yokectl()
        .args(["catalog", "modifiers"])
        .assert()
        .success()
        .stdout(predicates::str::contains("delay_on"))
        .stdout(predicates::str::contains("toggle"));
}
