use std::path::PathBuf;
use std::time::Duration;
use url::Url;

use crate::cache::Cache;
use crate::entry::{IndexEntry, IndexListing, parse_index};
use crate::source::ProfileSource;
use crate::url_transform::to_csv_export;
use crate::{COMMUNITY_INDEX_URL, IndexError};

pub const CACHE_DIR_ENV: &str = "YOKECTL_CACHE_DIR";

pub struct IndexClient {
    http: reqwest::Client,
    cache: Cache,
    index_url: String,
}

impl IndexClient {
    pub fn new() -> Result<Self, IndexError> {
        // YOKECTL_CACHE_DIR and YOKECTL_INDEX_URL let tests bypass the platform cache + URL.
        let cache = Cache::resolve_default().ok_or(IndexError::NoCacheDir)?;
        let index_url =
            std::env::var("YOKECTL_INDEX_URL").unwrap_or_else(|_| COMMUNITY_INDEX_URL.to_string());
        Ok(Self {
            http: reqwest::Client::builder()
                .user_agent(concat!("yokectl/", env!("CARGO_PKG_VERSION")))
                .build()?,
            cache,
            index_url,
        })
    }

    #[must_use]
    pub fn with_index_url(mut self, url: impl Into<String>) -> Self {
        self.index_url = url.into();
        self
    }

    #[must_use]
    pub fn with_cache(mut self, path: PathBuf, ttl: Duration) -> Self {
        self.cache = Cache::with_path(path, ttl);
        self
    }

    pub async fn list(&self, refresh: bool) -> Result<IndexListing, IndexError> {
        let cached = if refresh {
            None
        } else {
            self.cache.read_fresh().await?
        };
        let bytes = if let Some(b) = cached {
            b
        } else {
            let url = self
                .index_url
                .parse::<Url>()
                .map_err(|e| IndexError::InvalidUrl(e.to_string()))?;
            let b = self.fetch_url(&url).await?;
            if let Err(e) = self.cache.write(&b).await {
                tracing::warn!(?e, "cache write failed");
            }
            b
        };
        parse_index(&bytes)
    }

    pub async fn resolve(&self, name: &str) -> Result<IndexEntry, IndexError> {
        let listing = self.list(false).await?;
        let needle = name.to_lowercase();
        listing
            .entries
            .into_iter()
            .find(|e| e.name.to_lowercase() == needle)
            .ok_or_else(|| IndexError::NotFound(name.to_string()))
    }

    pub async fn fetch_profile(&self, src: ProfileSource) -> Result<Vec<u8>, IndexError> {
        match src {
            ProfileSource::LocalPath(p) => Ok(tokio::fs::read(p).await?),
            ProfileSource::Url(u) => fetch_url_with(&self.http, &to_csv_export(&u)?).await,
            ProfileSource::IndexEntry(name) => {
                let entry = self.resolve(&name).await?;
                fetch_url_with(&self.http, &to_csv_export(&entry.csv_url)?).await
            }
        }
    }

    async fn fetch_url(&self, url: &Url) -> Result<Vec<u8>, IndexError> {
        fetch_url_with(&self.http, url).await
    }
}

fn http_client() -> Result<reqwest::Client, IndexError> {
    Ok(reqwest::Client::builder()
        .user_agent(concat!("yokectl/", env!("CARGO_PKG_VERSION")))
        .build()?)
}

async fn fetch_url_with(http: &reqwest::Client, url: &Url) -> Result<Vec<u8>, IndexError> {
    let resp = http.get(url.clone()).send().await?;
    let status = resp.status();
    if !status.is_success() {
        return Err(IndexError::FetchFailed {
            url: url.clone(),
            status: status.as_u16(),
        });
    }
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    if let Some(ct) = &content_type
        && ct.to_ascii_lowercase().contains("text/html")
    {
        return Err(IndexError::HtmlResponse {
            url: url.clone(),
            content_type: ct.clone(),
        });
    }
    Ok(resp.bytes().await?.to_vec())
}

/// Fetch profile bytes for `src` without forcing a cache directory.
///
/// Only `IndexEntry` constructs an `IndexClient` (and therefore only that arm
/// needs `YOKECTL_CACHE_DIR` / `directories::ProjectDirs`). `LocalPath` reads
/// the file directly; `Url` runs a single anonymous HTTP GET. Tests can use
/// `IndexClient::new` directly when they want the cached / index-resolving
/// path.
pub async fn fetch_profile_bytes(src: ProfileSource) -> Result<Vec<u8>, IndexError> {
    match src {
        ProfileSource::LocalPath(p) => Ok(tokio::fs::read(p).await?),
        ProfileSource::Url(u) => {
            let http = http_client()?;
            fetch_url_with(&http, &to_csv_export(&u)?).await
        }
        ProfileSource::IndexEntry(_) => IndexClient::new()?.fetch_profile(src).await,
    }
}
