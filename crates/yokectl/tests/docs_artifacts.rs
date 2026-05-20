use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use assert_cmd::Command;
use tempfile::tempdir;
use walkdir::WalkDir;

fn yokectl() -> Command {
    Command::cargo_bin("yokectl").unwrap()
}

fn snapshot_dir(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    for entry in WalkDir::new(dir).into_iter().flatten() {
        if entry.file_type().is_file() {
            let rel = entry
                .path()
                .strip_prefix(dir)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            out.insert(rel, fs::read(entry.path()).unwrap());
        }
    }
    out
}

const TOP_LEAVES: &[&str] = &[
    "device",
    "debug",
    "watch",
    "list",
    "show",
    "validate",
    "pull",
    "push",
    "copy",
    "rename",
    "delete",
    "set-title",
    "set-preference",
    "unset-preference",
    "set-override",
    "unset-override",
    "set-binding",
    "clear-binding",
    "apply",
    "install",
    "completions",
    "docs",
];

const GROUPS: &[&str] = &["subprofile", "index", "catalog"];

const NESTED_LEAVES: &[(&str, &[&str])] = &[
    ("subprofile", &["add", "delete", "rename", "clone"]),
    ("index", &["list", "search", "show", "update"]),
    (
        "catalog",
        &["inputs", "outputs", "preferences", "modes", "channels"],
    ),
];

#[test]
fn docs_man_writes_root_and_leaf_pages() {
    let dir = tempdir().unwrap();
    yokectl()
        .args([
            "docs",
            "--format",
            "man",
            "--out",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success();
    let man = dir.path().join("man");

    let root = fs::read_to_string(man.join("yokectl.1")).unwrap();
    assert!(
        root.contains(".TH YOKECTL 1"),
        "root .TH missing in:\n{root}"
    );

    for sub in TOP_LEAVES.iter().chain(GROUPS.iter()) {
        let path = man.join(format!("yokectl-{sub}.1"));
        assert!(path.exists(), "missing man page: {path:?}");
    }
    for (group, leaves) in NESTED_LEAVES {
        for leaf in *leaves {
            let path = man.join(format!("yokectl-{group}-{leaf}.1"));
            assert!(path.exists(), "missing nested man page: {path:?}");
        }
    }
}

#[test]
fn docs_md_writes_single_reference_with_nested_headings() {
    let dir = tempdir().unwrap();
    yokectl()
        .args([
            "docs",
            "--format",
            "md",
            "--out",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success();
    let md = fs::read_to_string(dir.path().join("markdown/yokectl.md")).unwrap();
    assert!(md.starts_with("# yokectl"), "md head: {:?}", &md[..40]);

    for top in TOP_LEAVES.iter().chain(GROUPS.iter()) {
        let needle = format!("\n## {top}\n");
        assert!(md.contains(&needle), "missing heading: {needle:?}");
    }
    for (_, leaves) in NESTED_LEAVES {
        for leaf in *leaves {
            let needle = format!("\n### {leaf}\n");
            assert!(md.contains(&needle), "missing nested heading: {needle:?}");
        }
    }
}

#[test]
fn docs_man_is_idempotent() {
    let dir = tempdir().unwrap();
    let out_arg = dir.path().to_str().unwrap();
    yokectl()
        .args(["docs", "--format", "man", "--out", out_arg])
        .assert()
        .success();
    let first = snapshot_dir(&dir.path().join("man"));
    yokectl()
        .args(["docs", "--format", "man", "--out", out_arg])
        .assert()
        .success();
    let second = snapshot_dir(&dir.path().join("man"));
    assert_eq!(first, second, "man tree differs across re-runs");
}

#[test]
fn docs_md_is_idempotent() {
    let dir = tempdir().unwrap();
    let out_arg = dir.path().to_str().unwrap();
    let md_path = dir.path().join("markdown/yokectl.md");
    yokectl()
        .args(["docs", "--format", "md", "--out", out_arg])
        .assert()
        .success();
    let first = fs::read(&md_path).unwrap();
    yokectl()
        .args(["docs", "--format", "md", "--out", out_arg])
        .assert()
        .success();
    let second = fs::read(&md_path).unwrap();
    assert_eq!(first, second, "markdown differs across re-runs");
}

#[test]
fn docs_supports_both_formats_side_by_side() {
    let dir = tempdir().unwrap();
    let out_arg = dir.path().to_str().unwrap();
    yokectl()
        .args(["docs", "--format", "man", "--out", out_arg])
        .assert()
        .success();
    yokectl()
        .args(["docs", "--format", "md", "--out", out_arg])
        .assert()
        .success();
    assert!(dir.path().join("man/yokectl.1").is_file());
    assert!(dir.path().join("markdown/yokectl.md").is_file());
}
