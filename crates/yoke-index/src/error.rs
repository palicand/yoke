use thiserror::Error;
use url::Url;

#[derive(Error, Debug)]
pub enum IndexError {
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("index URL not parseable: {0}")]
    InvalidUrl(String),
    #[error("fetch failed: {url}: HTTP {status}")]
    FetchFailed { url: Url, status: u16 },
    #[error("index format unexpected: {0}")]
    IndexFormat(String),
    #[error("no index entry matching name: {0}")]
    NotFound(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}
