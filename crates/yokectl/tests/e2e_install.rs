mod common;

use common::yokectl;
use tempfile::tempdir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const FIXTURE: &str =
    "QuadStick Configuration,Version 1.4,Mock,Destiny 2,,\n,,,,\n*Main,sip_puff,,A,inputs\n";

#[tokio::test(flavor = "current_thread")]
async fn install_list_show_set_binding_pull_diff() {
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

    let dir = tempdir().unwrap();
    let vol = dir.path().to_str().unwrap();
    let cache_dir = tempdir().unwrap();
    let index_url = format!("{}/index.csv", server.uri());

    // YOKECTL_CACHE_DIR is what IndexClient::new actually honors; XDG_CACHE_HOME
    // is ignored on macOS where directories::ProjectDirs routes to
    // ~/Library/Caches/. Pointing the cache at a fresh tempdir per test keeps
    // the install index hermetic.
    yokectl()
        .env("YOKECTL_INDEX_URL", &index_url)
        .env("YOKECTL_CACHE_DIR", cache_dir.path())
        .args(["--fake-volume", vol, "install", "Destiny 2"])
        .assert()
        .success();

    yokectl()
        .args(["--fake-volume", vol, "list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("destiny_2"));

    yokectl()
        .args(["--fake-volume", vol, "show", "destiny_2"])
        .assert()
        .success();

    // A separate tempdir for the pull destination keeps the volume root clean
    // — pulling into the volume itself would leave a non-.csv side file there.
    let pull_dir = tempdir().unwrap();
    let dest = pull_dir.path().join("pulled.csv");
    yokectl()
        .args([
            "--fake-volume",
            vol,
            "pull",
            "destiny_2",
            dest.to_str().unwrap(),
        ])
        .assert()
        .success();
    assert_eq!(std::fs::read(&dest).unwrap(), FIXTURE.as_bytes());
}
