mod common;
use common::{seed_profile, yokectl};
use tempfile::tempdir;

// Top line, then a Preferences section (rows: header, filename, column labels, entries),
// then a sub-profile section with one override row.
const FIXTURE: &str = "QuadStick Configuration,Version 1.4,Mock,Default\r\n\
Preferences,\r\n\
default.csv,,,,\r\n\
Preference,Value,Units,Description,\r\n\
volume,55,,,System Volume\r\n\
\r\n\
Profile Name,Main,Mouse,usb\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
volume,override,70,\r\n\
\r\n";

#[test]
fn preferences_effective_lists_overrides() {
    let dir = tempdir().unwrap();
    seed_profile(dir.path(), "default.csv", FIXTURE);
    yokectl()
        .arg("--fake-volume")
        .arg(dir.path())
        .arg("preferences")
        .arg("default")
        .assert()
        .success()
        .stdout(predicates::str::contains("Top-level:"))
        .stdout(predicates::str::contains("Main:"));
}

#[test]
fn preferences_raw_shows_overrides_block() {
    let dir = tempdir().unwrap();
    seed_profile(dir.path(), "default.csv", FIXTURE);
    yokectl()
        .arg("--fake-volume")
        .arg(dir.path())
        .arg("preferences")
        .arg("default")
        .arg("--raw")
        .assert()
        .success()
        .stdout(predicates::str::contains("Main (overrides):"));
}

#[test]
fn preferences_missing_sub_profile_exits_5() {
    let dir = tempdir().unwrap();
    seed_profile(dir.path(), "default.csv", FIXTURE);
    yokectl()
        .arg("--fake-volume")
        .arg(dir.path())
        .arg("preferences")
        .arg("default")
        .arg("--sub-profile")
        .arg("Nope")
        .assert()
        .code(5);
}
