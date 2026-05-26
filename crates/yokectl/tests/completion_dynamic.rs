mod common;
use common::yokectl;

#[test]
fn dynamic_completion_script_is_non_empty_for_each_shell() {
    for shell in ["bash", "zsh", "fish", "elvish", "powershell"] {
        let out = yokectl()
            .env("COMPLETE", shell)
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        assert!(
            !out.is_empty(),
            "shell {shell} produced empty completion script"
        );
    }
}

// Bash completion protocol: COMPLETE=bash + _CLAP_COMPLETE_INDEX=N drives candidate emission.
// Args after -- are: <binary> <word0> <word1> ... where N indexes which word is being completed.

#[test]
fn profile_name_completer_lists_fake_volume_contents() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("foo.csv"), b"x").unwrap();
    // Argument layout: yokectl(0) --fake-volume(1) <dir>(2) show(3) ""(4)
    let out = yokectl()
        .env("COMPLETE", "bash")
        .env("_CLAP_COMPLETE_INDEX", "4")
        .arg("--")
        .arg("yokectl")
        .arg("--fake-volume")
        .arg(dir.path())
        .arg("show")
        .arg("")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8_lossy(&out);
    assert!(s.contains("foo"), "candidates did not include 'foo': {s}");
}

#[test]
fn profile_name_completer_returns_silently_when_volume_missing() {
    // Argument layout: yokectl(0) --fake-volume(1) <missing-path>(2) show(3) ""(4)
    let out = yokectl()
        .env("COMPLETE", "bash")
        .env("_CLAP_COMPLETE_INDEX", "4")
        .arg("--")
        .arg("yokectl")
        .arg("--fake-volume")
        .arg("/path/that/does/not/exist")
        .arg("show")
        .arg("")
        .assert()
        .success()
        .get_output()
        .clone();
    assert!(
        out.stderr.is_empty(),
        "completion path wrote to stderr: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn sub_profile_completer_lists_sub_profiles_of_target() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("default.csv"), common::FIXTURE_WITH_SUB).unwrap();
    // Argument layout: yokectl(0) --fake-volume(1) <dir>(2) bindings(3) default(4) --sub-profile(5) ""(6)
    let out = yokectl()
        .env("COMPLETE", "bash")
        .env("_CLAP_COMPLETE_INDEX", "6")
        .arg("--")
        .arg("yokectl")
        .arg("--fake-volume")
        .arg(dir.path())
        .arg("bindings")
        .arg("default")
        .arg("--sub-profile")
        .arg("")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8_lossy(&out);
    assert!(
        s.contains("Main"),
        "sub-profile candidates did not include 'Main': {s}"
    );
}

#[test]
fn index_entry_completer_lists_entries_from_cache_dir_env() {
    let dir = tempfile::tempdir().unwrap();
    // A probe name the developer's real platform cache will not contain, so a pass
    // proves the env-var dir was read rather than the platform default.
    std::fs::write(
        dir.path().join("index.csv"),
        b"Name,CSV URL\nYokectlCacheEnvProbe,https://x.example/probe.csv\n",
    )
    .unwrap();
    // Argument layout: yokectl(0) install(1) ""(2)
    let out = yokectl()
        .env("COMPLETE", "bash")
        .env("_CLAP_COMPLETE_INDEX", "2")
        .env("YOKECTL_CACHE_DIR", dir.path())
        .arg("--")
        .arg("yokectl")
        .arg("install")
        .arg("")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8_lossy(&out);
    assert!(
        s.contains("YokectlCacheEnvProbe"),
        "install completion did not read YOKECTL_CACHE_DIR: {s}"
    );
}

#[test]
fn index_entry_completer_returns_silently_when_cache_missing() {
    let dir = tempfile::tempdir().unwrap();
    // Argument layout: yokectl(0) install(1) ""(2)
    // XDG_CACHE_HOME is not honoured on macOS by directories::ProjectDirs.
    // We set YOKECTL_CACHE_DIR (the env var that yoke-index reads) to point at an empty dir.
    let out = yokectl()
        .env("COMPLETE", "bash")
        .env("_CLAP_COMPLETE_INDEX", "2")
        .env("YOKECTL_CACHE_DIR", dir.path())
        .arg("--")
        .arg("yokectl")
        .arg("install")
        .arg("")
        .assert()
        .success()
        .get_output()
        .clone();
    assert!(
        out.stderr.is_empty(),
        "completion path wrote to stderr: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );
}
