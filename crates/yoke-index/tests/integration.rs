use std::path::Path;
use std::time::Duration;

use tempfile::tempdir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use yoke_index::{IndexClient, ProfileSource};

const INDEX_CSV: &str = "Name,CSV URL\nDestiny 2,REPLACE_ME\n";
const PROFILE_CSV: &str = "QuadStick Configuration,Version 1.4,Mock,Destiny 2,,";

fn client_against(server: &MockServer, cache_dir: &Path) -> IndexClient {
    let cache_path = cache_dir.join("idx.csv");
    IndexClient::new()
        .unwrap()
        .with_index_url(format!("{}/index.csv", server.uri()))
        .with_cache(cache_path, Duration::from_mins(1))
}

#[tokio::test(flavor = "current_thread")]
async fn list_fetches_and_caches_index() {
    let server = MockServer::start().await;
    let dir = tempdir().unwrap();
    let body = INDEX_CSV.replace("REPLACE_ME", &format!("{}/d2.csv", server.uri()));
    Mock::given(method("GET"))
        .and(path("/index.csv"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .expect(1)
        .mount(&server)
        .await;

    let c = client_against(&server, dir.path());
    let entries = c.list(false).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "Destiny 2");
    let _entries2 = c.list(false).await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn fetch_profile_by_index_entry_chains_two_gets() {
    let server = MockServer::start().await;
    let dir = tempdir().unwrap();
    let body = INDEX_CSV.replace("REPLACE_ME", &format!("{}/d2.csv", server.uri()));
    Mock::given(method("GET"))
        .and(path("/index.csv"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/d2.csv"))
        .respond_with(ResponseTemplate::new(200).set_body_string(PROFILE_CSV))
        .mount(&server)
        .await;
    let c = client_against(&server, dir.path());
    let bytes = c
        .fetch_profile(ProfileSource::IndexEntry("Destiny 2".into()))
        .await
        .unwrap();
    assert_eq!(bytes, PROFILE_CSV.as_bytes());
}

#[tokio::test(flavor = "current_thread")]
async fn fetch_profile_local_path_skips_network() {
    let server = MockServer::start().await;
    let dir = tempdir().unwrap();
    let p = dir.path().join("local.csv");
    tokio::fs::write(&p, PROFILE_CSV).await.unwrap();
    let c = client_against(&server, dir.path());
    let bytes = c.fetch_profile(ProfileSource::LocalPath(p)).await.unwrap();
    assert_eq!(bytes, PROFILE_CSV.as_bytes());
}

#[tokio::test(flavor = "current_thread")]
async fn fetch_404_returns_fetch_failed() {
    let server = MockServer::start().await;
    let dir = tempdir().unwrap();
    Mock::given(method("GET"))
        .and(path("/index.csv"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let c = client_against(&server, dir.path());
    let err = c.list(true).await.unwrap_err();
    assert!(matches!(
        err,
        yoke_index::IndexError::FetchFailed { status: 404, .. }
    ));
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "opt-in: requires YOKE_REAL_NETWORK=1 and live internet"]
async fn real_community_index_fetches_when_env_set() {
    if std::env::var("YOKE_REAL_NETWORK").as_deref() != Ok("1") {
        return;
    }
    let dir = tempdir().unwrap();
    let c = IndexClient::new()
        .unwrap()
        .with_cache(dir.path().join("idx.csv"), Duration::from_mins(1));
    let entries = c.list(true).await.expect("network fetch failed");
    assert!(!entries.is_empty(), "index empty");
    eprintln!("fetched {} entries", entries.len());
}
