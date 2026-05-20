use std::path::PathBuf;
use std::time::Duration;
use url::Url;

use crate::cache::Cache;
use crate::entry::{IndexEntry, parse_index};
use crate::source::ProfileSource;
use crate::url_transform::to_csv_export;
use crate::{COMMUNITY_INDEX_URL, IndexError};

pub struct IndexClient {
    http: reqwest::Client,
    cache: Cache,
    index_url: String,
}

impl IndexClient {
    pub fn new() -> Result<Self, IndexError> {
        // YOKECTL_CACHE_DIR lets tests bypass the platform cache location.
        let cache = if let Some(p) = std::env::var_os("YOKECTL_CACHE_DIR") {
            Cache::with_path(PathBuf::from(p).join("index.csv"), Duration::from_hours(24))
        } else {
            Cache::default().ok_or(IndexError::NoCacheDir)?
        };
        Ok(Self {
            http: reqwest::Client::builder()
                .user_agent(concat!("yokectl/", env!("CARGO_PKG_VERSION")))
                .build()?,
            cache,
            index_url: COMMUNITY_INDEX_URL.to_string(),
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

    pub async fn list(&self, refresh: bool) -> Result<Vec<IndexEntry>, IndexError> {
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
        let (entries, _skipped) = parse_index(&bytes)?;
        Ok(entries)
    }

    pub async fn resolve(&self, name: &str) -> Result<IndexEntry, IndexError> {
        let entries = self.list(false).await?;
        entries
            .into_iter()
            .find(|e| e.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| IndexError::NotFound(name.to_string()))
    }

    pub async fn fetch_profile(&self, src: ProfileSource) -> Result<Vec<u8>, IndexError> {
        match src {
            ProfileSource::LocalPath(p) => Ok(tokio::fs::read(p).await?),
            ProfileSource::Url(u) => self.fetch_url(&to_csv_export(&u)?).await,
            ProfileSource::IndexEntry(name) => {
                let entry = self.resolve(&name).await?;
                self.fetch_url(&to_csv_export(&entry.csv_url)?).await
            }
        }
    }

    async fn fetch_url(&self, url: &Url) -> Result<Vec<u8>, IndexError> {
        let resp = self.http.get(url.clone()).send().await?;
        let status = resp.status();
        if !status.is_success() {
            return Err(IndexError::FetchFailed {
                url: url.clone(),
                status: status.as_u16(),
            });
        }
        Ok(resp.bytes().await?.to_vec())
    }
}
