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
        .arg("Main")
        .assert()
        .success()
        .stdout(predicates::str::contains("Main"));
}

#[test]
fn bindings_missing_sub_profile_exits_5() {
    let dir = tempdir().unwrap();
    seed_profile(dir.path(), "default.csv", FIXTURE_WITH_SUB);
    yokectl()
        .arg("--fake-volume")
        .arg(dir.path())
        .arg("bindings")
        .arg("default")
        .arg("--sub-profile")
        .arg("Nope")
        .assert()
        .code(5);
}
