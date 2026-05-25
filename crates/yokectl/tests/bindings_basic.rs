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
