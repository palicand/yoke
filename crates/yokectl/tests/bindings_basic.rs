mod common;
use common::{FIXTURE_WITH_SUB, seed_profile, yokectl};
use tempfile::tempdir;

#[test]
fn bindings_lists_for_default_profile() {
    let dir = tempdir().unwrap();
    seed_profile(dir.path(), "default.csv", FIXTURE_WITH_SUB);
    yokectl()
        .arg("--fake-volume")
        .arg(dir.path())
        .arg("bindings")
        .arg("default")
        .assert()
        .success()
        .stdout(predicates::str::contains("Main (mode="));
}

#[test]
fn bindings_filter_to_single_sub_profile() {
    let dir = tempdir().unwrap();
    seed_profile(dir.path(), "default.csv", FIXTURE_WITH_SUB);
    yokectl()
        .arg("--fake-volume")
        .arg(dir.path())
        .arg("bindings")
        .arg("default")
        .arg("--sub-profile")
        .arg("0")
        .assert()
        .success()
        .stdout(predicates::str::contains("Main"));
}

#[test]
fn bindings_out_of_range_sub_profile_exits_2() {
    let dir = tempdir().unwrap();
    seed_profile(dir.path(), "default.csv", FIXTURE_WITH_SUB);
    let out = yokectl()
        .arg("--json")
        .arg("--fake-volume")
        .arg(dir.path())
        .arg("bindings")
        .arg("default")
        .arg("--sub-profile")
        .arg("9")
        .assert()
        .code(2)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["error"]["code"], "cli-subprofile-index-out-of-range");
}
