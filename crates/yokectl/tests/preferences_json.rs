mod common;
use common::{seed_profile, yokectl};
use tempfile::tempdir;

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

fn run(args: &[&str]) -> serde_json::Value {
    let dir = tempdir().unwrap();
    seed_profile(dir.path(), "default.csv", FIXTURE);
    let mut cmd = yokectl();
    cmd.arg("--json").arg("--fake-volume").arg(dir.path());
    for a in args {
        cmd.arg(a);
    }
    let out = cmd.assert().success();
    serde_json::from_slice(&out.get_output().stdout).expect("valid JSON")
}

#[test]
fn preferences_json_effective_default() {
    let v = run(&["preferences", "default"]);
    insta::assert_json_snapshot!("effective_default", v);
}

#[test]
fn preferences_json_raw() {
    let v = run(&["preferences", "default", "--raw"]);
    insta::assert_json_snapshot!("raw", v);
}

#[test]
fn preferences_json_filtered() {
    let v = run(&["preferences", "default", "--sub-profile", "0"]);
    insta::assert_json_snapshot!("filtered", v);
}
