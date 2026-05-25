mod common;
use common::{FIXTURE_WITH_SUB, seed_profile, yokectl};
use tempfile::tempdir;

#[test]
fn bindings_json_default() {
    let dir = tempdir().unwrap();
    seed_profile(dir.path(), "default.csv", FIXTURE_WITH_SUB);
    let out = yokectl()
        .arg("--json")
        .arg("--fake-volume")
        .arg(dir.path())
        .arg("bindings")
        .arg("default")
        .assert()
        .success();
    let stdout: serde_json::Value =
        serde_json::from_slice(&out.get_output().stdout).expect("valid JSON");
    insta::assert_json_snapshot!("default", stdout);
}

#[test]
fn bindings_json_filtered() {
    let dir = tempdir().unwrap();
    seed_profile(dir.path(), "default.csv", FIXTURE_WITH_SUB);
    let out = yokectl()
        .arg("--json")
        .arg("--fake-volume")
        .arg(dir.path())
        .arg("bindings")
        .arg("default")
        .arg("--sub-profile")
        .arg("Main")
        .assert()
        .success();
    let stdout: serde_json::Value =
        serde_json::from_slice(&out.get_output().stdout).expect("valid JSON");
    insta::assert_json_snapshot!("filtered", stdout);
}
