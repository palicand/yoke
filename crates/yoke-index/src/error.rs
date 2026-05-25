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
    #[error("fetch returned HTML where CSV was expected: {url} (Content-Type: {content_type})")]
    HtmlResponse { url: Url, content_type: String },
    #[error("index format unexpected: {0}")]
    IndexFormat(String),
    #[error("no index entry matching name: {0}")]
    NotFound(String),
    #[error("no cache directory available; set YOKECTL_CACHE_DIR to override")]
    NoCacheDir,
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}
