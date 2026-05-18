use assert_cmd::Command;
use tempfile::tempdir;

fn yokectl() -> Command {
    Command::cargo_bin("yokectl").unwrap()
}

const FIXTURE: &str =
    "QuadStick Configuration,Version 1.4,Mock,Default,,\n,,,,\n*Main,sip_puff,,A,inputs\n";

const FIXTURE_WITH_SUB: &str = "QuadStick Configuration,Version 1.4,Mock,Default\r\n\
Profile Name,Main,Mouse,usb\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mouse_left,normal,left,\r\n\
\r\n";

fn run_json(args: &[&str], dir: &std::path::Path) -> serde_json::Value {
    let mut cmd = yokectl();
    cmd.args(["--fake-volume", dir.to_str().unwrap(), "--json"]);
    cmd.args(args);
    let out = cmd.assert().success().get_output().stdout.clone();
    serde_json::from_slice(&out).unwrap()
}

#[test]
fn device_json_snapshot() {
    let dir = tempdir().unwrap();
    let v = run_json(&["device"], dir.path());
    insta::assert_json_snapshot!(v, {
        ".state.mount_point" => "[REDACTED]",
        ".state.label" => "[REDACTED]",
    });
}

#[test]
fn debug_json_snapshot() {
    let dir = tempdir().unwrap();
    let v = run_json(&["debug"], dir.path());
    insta::assert_json_snapshot!(v, {
        ".device.mount_point" => "[REDACTED]",
        ".device.label" => "[REDACTED]",
        ".mount.mount_point" => "[REDACTED]",
        ".mount.label" => "[REDACTED]",
    });
}

#[test]
fn list_json_snapshot() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("default.csv"), FIXTURE).unwrap();
    let v = run_json(&["list"], dir.path());
    insta::assert_json_snapshot!(v, {
        ".profiles[].byte_len" => "[REDACTED]",
    });
}

#[test]
fn show_json_snapshot() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("default.csv"), FIXTURE_WITH_SUB).unwrap();
    let v = run_json(&["show", "default"], dir.path());
    insta::assert_json_snapshot!(v);
}

#[test]
fn validate_json_snapshot() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("default.csv"), FIXTURE).unwrap();
    let v = run_json(&["validate", "default"], dir.path());
    insta::assert_json_snapshot!(v);
}

#[test]
fn set_preference_json_snapshot() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("default.csv"), FIXTURE).unwrap();
    let v = run_json(&["set-preference", "default", "volume", "55"], dir.path());
    insta::assert_json_snapshot!(v);
}

#[test]
fn set_title_json_snapshot() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("default.csv"), FIXTURE).unwrap();
    let v = run_json(&["set-title", "default", "Renamed"], dir.path());
    insta::assert_json_snapshot!(v);
}

#[test]
fn pull_json_snapshot() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("default.csv"), FIXTURE).unwrap();
    let dest = dir.path().join("out.csv");
    let v = run_json(&["pull", "default", dest.to_str().unwrap()], dir.path());
    insta::assert_json_snapshot!(v, {
        ".dest" => "[REDACTED]",
        ".bytes" => "[REDACTED]",
    });
}

#[test]
fn copy_json_snapshot() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("default.csv"), FIXTURE).unwrap();
    let v = run_json(&["copy", "default", "alt"], dir.path());
    insta::assert_json_snapshot!(v);
}

#[test]
fn rename_json_snapshot() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("default.csv"), FIXTURE).unwrap();
    let v = run_json(&["rename", "default", "renamed"], dir.path());
    insta::assert_json_snapshot!(v);
}

#[test]
fn delete_json_snapshot() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("default.csv"), FIXTURE).unwrap();
    let v = run_json(&["delete", "default", "--force"], dir.path());
    insta::assert_json_snapshot!(v);
}

#[test]
fn subprofile_add_json_snapshot() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("default.csv"), FIXTURE_WITH_SUB).unwrap();
    let v = run_json(
        &[
            "subprofile",
            "add",
            "default",
            "Alt",
            "--mode",
            "Mouse",
            "--channel",
            "usb",
        ],
        dir.path(),
    );
    insta::assert_json_snapshot!(v);
}

#[test]
fn catalog_inputs_json_snapshot() {
    let dir = tempdir().unwrap();
    let v = run_json(&["catalog", "inputs"], dir.path());
    insta::assert_json_snapshot!(v);
}

#[test]
fn catalog_outputs_json_snapshot() {
    let dir = tempdir().unwrap();
    let v = run_json(&["catalog", "outputs"], dir.path());
    insta::assert_json_snapshot!(v);
}

#[test]
fn catalog_preferences_json_snapshot() {
    let dir = tempdir().unwrap();
    let v = run_json(&["catalog", "preferences"], dir.path());
    insta::assert_json_snapshot!(v);
}

#[test]
fn catalog_modes_json_snapshot() {
    let dir = tempdir().unwrap();
    let v = run_json(&["catalog", "modes"], dir.path());
    insta::assert_json_snapshot!(v);
}

#[test]
fn catalog_channels_json_snapshot() {
    let dir = tempdir().unwrap();
    let v = run_json(&["catalog", "channels"], dir.path());
    insta::assert_json_snapshot!(v);
}
