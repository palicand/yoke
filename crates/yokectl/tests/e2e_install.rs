use assert_cmd::Command;
use tempfile::tempdir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn yokectl() -> Command {
    Command::cargo_bin("yokectl").unwrap()
}

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

    // Isolated XDG_CACHE_HOME keeps the index cache out of the user's home and avoids
    // a hot cache from a prior run masking the wiremock fetch.
    yokectl()
        .env("YOKECTL_INDEX_URL", &index_url)
        .env("XDG_CACHE_HOME", cache_dir.path())
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

    let dest = dir.path().join("pulled.csv");
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
