use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use crate::IndexError;

pub struct Cache {
    pub path: PathBuf,
    pub ttl: Duration,
}

impl Cache {
    // Plan specifies `default() -> Option<Self>` because the project-dirs lookup can fail.
    // This intentionally shadows the `Default` trait signature; see the plan for context.
    #[allow(clippy::should_implement_trait)]
    pub fn default() -> Option<Self> {
        directories::ProjectDirs::from("com", "Yoke", "yokectl").map(|p| Self {
            path: p.cache_dir().join("index.csv"),
            ttl: Duration::from_hours(24),
        })
    }

    #[must_use]
    pub const fn with_path(path: PathBuf, ttl: Duration) -> Self {
        Self { path, ttl }
    }

    pub async fn read_fresh(&self) -> Result<Option<Vec<u8>>, IndexError> {
        if !self.path.exists() {
            return Ok(None);
        }
        let meta = tokio::fs::metadata(&self.path).await?;
        let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let age = SystemTime::now()
            .duration_since(modified)
            .unwrap_or(Duration::ZERO);
        if age > self.ttl {
            return Ok(None);
        }
        let bytes = tokio::fs::read(&self.path).await?;
        Ok(Some(bytes))
    }

    pub async fn write(&self, bytes: &[u8]) -> Result<(), IndexError> {
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&self.path, bytes).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test(flavor = "current_thread")]
    async fn read_fresh_returns_none_when_missing() {
        let dir = tempdir().unwrap();
        let c = Cache::with_path(dir.path().join("idx.csv"), Duration::from_mins(1));
        assert!(c.read_fresh().await.unwrap().is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn read_fresh_returns_some_when_within_ttl() {
        let dir = tempdir().unwrap();
        let c = Cache::with_path(dir.path().join("idx.csv"), Duration::from_mins(1));
        c.write(b"hello").await.unwrap();
        assert_eq!(
            c.read_fresh().await.unwrap().as_deref(),
            Some(b"hello" as &[u8])
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn read_fresh_returns_none_when_stale() {
        let dir = tempdir().unwrap();
        let c = Cache::with_path(dir.path().join("idx.csv"), Duration::from_nanos(1));
        c.write(b"hello").await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(c.read_fresh().await.unwrap().is_none());
    }
}
