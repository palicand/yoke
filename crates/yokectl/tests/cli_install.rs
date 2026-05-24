mod common;

use common::yokectl;
use predicates::prelude::*;
use tempfile::tempdir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const FIXTURE: &str =
    "QuadStick Configuration,Version 1.4,Mock,D2,,\n,,,,\n*Main,sip_puff,,A,inputs\n";

#[tokio::test(flavor = "current_thread")]
async fn install_from_url_writes_to_volume() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/d2.csv"))
        .respond_with(ResponseTemplate::new(200).set_body_string(FIXTURE))
        .mount(&server)
        .await;
    let dir = tempdir().unwrap();
    yokectl()
        .args([
            "--fake-volume",
            dir.path().to_str().unwrap(),
            "install",
            &format!("{}/d2.csv", server.uri()),
            "--as",
            "d2",
        ])
        .assert()
        .success();
    assert_eq!(
        std::fs::read(dir.path().join("d2.csv")).unwrap(),
        FIXTURE.as_bytes()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn install_from_local_file_skips_network() {
    let dir = tempdir().unwrap();
    let local = dir.path().join("local.csv");
    std::fs::write(&local, FIXTURE).unwrap();
    yokectl()
        .args([
            "--fake-volume",
            dir.path().to_str().unwrap(),
            "install",
            local.to_str().unwrap(),
            "--as",
            "from-local",
        ])
        .assert()
        .success();
    assert_eq!(
        std::fs::read(dir.path().join("from-local.csv")).unwrap(),
        FIXTURE.as_bytes()
    );
}

#[test]
fn install_local_file_works_without_home_or_cache_dir() {
    let dir = tempdir().unwrap();
    let local = dir.path().join("local.csv");
    std::fs::write(&local, FIXTURE).unwrap();
    yokectl()
        .env_remove("HOME")
        .args([
            "--fake-volume",
            dir.path().to_str().unwrap(),
            "install",
            local.to_str().unwrap(),
            "--as",
            "no-home",
        ])
        .assert()
        .success();
}

#[tokio::test(flavor = "current_thread")]
async fn second_install_without_as_requires_force_then_force_overwrites() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/d2.csv"))
        .respond_with(ResponseTemplate::new(200).set_body_string(FIXTURE))
        .mount(&server)
        .await;
    let dir = tempdir().unwrap();
    let vol = dir.path().to_str().unwrap();
    let url = format!("{}/d2.csv", server.uri());
    yokectl()
        .args(["--fake-volume", vol, "install", &url])
        .assert()
        .success();
    yokectl()
        .args(["--json", "--fake-volume", vol, "install", &url])
        .assert()
        .failure()
        .stdout(predicate::str::contains("cli-requires-force"));
    yokectl()
        .args(["--fake-volume", vol, "install", &url, "--force"])
        .assert()
        .success();
}

#[tokio::test(flavor = "current_thread")]
async fn second_install_with_as_silently_overwrites() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/d2.csv"))
        .respond_with(ResponseTemplate::new(200).set_body_string(FIXTURE))
        .mount(&server)
        .await;
    let dir = tempdir().unwrap();
    let vol = dir.path().to_str().unwrap();
    let url = format!("{}/d2.csv", server.uri());
    for _ in 0..2 {
        yokectl()
            .args(["--fake-volume", vol, "install", &url, "--as", "explicit"])
            .assert()
            .success();
    }
}

#[test]
fn install_dry_run_rejects_invalid_dest_name() {
    let dir = tempdir().unwrap();
    let local = dir.path().join("ok.csv");
    std::fs::write(&local, FIXTURE).unwrap();
    yokectl()
        .args([
            "--json",
            "--fake-volume",
            dir.path().to_str().unwrap(),
            "install",
            local.to_str().unwrap(),
            "--as",
            "bad:name",
            "--dry-run",
        ])
        .assert()
        .failure()
        .stdout(predicate::str::contains("invalid-name"));
}

#[tokio::test(flavor = "current_thread")]
async fn install_bare_name_does_not_shadow_with_cwd_file() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/index.csv"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(format!("Name,CSV URL\nDestiny 2,{}/d2.csv\n", server.uri())),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/d2.csv"))
        .respond_with(ResponseTemplate::new(200).set_body_string(FIXTURE))
        .mount(&server)
        .await;
    // A cwd file named exactly like the bare token MUST NOT shadow the index
    // lookup; classify is path-shape-driven, not existence-driven.
    let cwd = tempdir().unwrap();
    std::fs::write(cwd.path().join("Destiny 2"), b"cwd-bytes").unwrap();
    let vol = tempdir().unwrap();
    let cache = tempdir().unwrap();
    yokectl()
        .current_dir(cwd.path())
        .env("YOKECTL_INDEX_URL", format!("{}/index.csv", server.uri()))
        .env("YOKECTL_CACHE_DIR", cache.path())
        .args([
            "--fake-volume",
            vol.path().to_str().unwrap(),
            "install",
            "Destiny 2",
        ])
        .assert()
        .success();
    assert_eq!(
        std::fs::read(vol.path().join("destiny_2.csv")).unwrap(),
        FIXTURE.as_bytes()
    );
}
