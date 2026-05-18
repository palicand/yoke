use assert_cmd::Command;
use tempfile::tempdir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn yokectl() -> Command {
    Command::cargo_bin("yokectl").unwrap()
}

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
