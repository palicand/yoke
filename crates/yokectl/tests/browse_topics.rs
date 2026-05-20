use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;

fn yokectl() -> Command {
    Command::cargo_bin("yokectl").unwrap()
}

fn run_json(args: &[&str]) -> Value {
    let out = yokectl().args(args).assert().success().get_output().clone();
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn topic_lists_known_slugs() {
    let v = run_json(&["--json", "topic"]);
    let topics = v["topics"].as_array().unwrap();
    let slugs: Vec<&str> = topics.iter().map(|t| t["slug"].as_str().unwrap()).collect();
    for expected in [
        "binding-model",
        "sub-profiles",
        "sip-puff",
        "preferences",
        "install-sources",
    ] {
        assert!(slugs.contains(&expected), "missing topic: {expected}");
    }
}

#[test]
fn topic_show_returns_markdown_body() {
    let v = run_json(&["--json", "topic", "binding-model"]);
    assert_eq!(v["slug"], "binding-model");
    let body = v["body"].as_str().unwrap();
    assert!(body.starts_with("# "));
    assert!(body.lines().count() >= 20);
}

#[test]
fn topic_show_human_emits_raw_markdown() {
    yokectl()
        .args(["topic", "binding-model"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("# Binding model"));
}

#[test]
fn topic_unknown_slug_errors_with_suggestion() {
    yokectl()
        .args(["--json", "topic", "bindng-model"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("binding-model"));
}

#[test]
fn manual_list_returns_root_and_topics() {
    let v = run_json(&["--json", "manual"]);
    assert!(v["root"].as_str().unwrap().contains("configuration.htm"));
    let topics = v["topics"].as_array().unwrap();
    assert!(!topics.is_empty());
    for t in topics {
        let url = t["url"].as_str().unwrap();
        assert!(url.starts_with("https://"), "bad url: {url}");
        assert!(
            std::path::Path::new(url)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("htm")),
            "bad url: {url}"
        );
    }
}

#[test]
fn manual_known_topic_resolves_to_url() {
    let v = run_json(&["--json", "manual", "preferences"]);
    assert_eq!(v["slug"], "preferences");
    assert!(v["url"].as_str().unwrap().ends_with("preferences.htm"));
}

#[test]
fn manual_unknown_topic_errors_with_suggestion() {
    yokectl()
        .args(["--json", "manual", "preferencs"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("preferences"));
}

#[test]
fn index_browse_json_emits_html_url_without_launching() {
    let v = run_json(&["--json", "index", "browse"]);
    let url = v["url"].as_str().unwrap();
    assert!(url.contains("pubhtml"), "expected pubhtml URL, got {url}");
    assert!(!url.contains("output=csv"), "should not be the CSV form");
}
