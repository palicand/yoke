use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use tempfile::NamedTempFile;

use crate::IndexError;

const INDEX_FILENAME: &str = "index.csv";

pub struct Cache {
    pub(crate) path: PathBuf,
    pub(crate) ttl: Duration,
}

impl Cache {
    pub const DEFAULT_TTL: Duration = Duration::from_hours(24);

    pub fn from_project_dirs() -> Option<Self> {
        directories::ProjectDirs::from("com", "Yoke", "yokectl")
            .map(|p| Self::default_in(p.cache_dir()))
    }

    #[must_use]
    pub fn default_in(base: &Path) -> Self {
        Self {
            path: base.join(INDEX_FILENAME),
            ttl: Self::DEFAULT_TTL,
        }
    }

    #[must_use]
    pub const fn with_path(path: PathBuf, ttl: Duration) -> Self {
        Self { path, ttl }
    }

    pub async fn read_fresh(&self) -> Result<Option<Vec<u8>>, IndexError> {
        let meta = match tokio::fs::metadata(&self.path).await {
            Ok(m) => m,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(IndexError::Io(e)),
        };
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
        // NamedTempFile gets a unique name in the destination dir and is auto-deleted
        // if persist() fails or the process dies mid-write, so concurrent writers can't
        // trample each other and a torn write never becomes visible.
        let dest = self.path.clone();
        let bytes = bytes.to_vec();
        tokio::task::spawn_blocking(move || -> io::Result<()> {
            let parent = dest.parent().unwrap_or_else(|| Path::new("."));
            let mut tmp = NamedTempFile::new_in(parent)?;
            tmp.write_all(&bytes)?;
            tmp.persist(&dest).map_err(|e| e.error)?;
            Ok(())
        })
        .await
        .map_err(|e| IndexError::Io(io::Error::other(e)))??;
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
