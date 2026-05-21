use std::path::PathBuf;
use url::Url;

use crate::IndexError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileSource {
    LocalPath(PathBuf),
    Url(Url),
    IndexEntry(String),
}

impl ProfileSource {
    pub fn classify(raw: &str) -> Result<Self, IndexError> {
        let path = PathBuf::from(raw);
        if path.exists() {
            return Ok(Self::LocalPath(path));
        }
        if raw.starts_with("http://") || raw.starts_with("https://") {
            let url = Url::parse(raw).map_err(|e| IndexError::InvalidUrl(e.to_string()))?;
            return Ok(Self::Url(url));
        }
        Ok(Self::IndexEntry(raw.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn existing_path_classifies_as_local() {
        let f = NamedTempFile::new().unwrap();
        let s = ProfileSource::classify(f.path().to_str().unwrap()).unwrap();
        assert!(matches!(s, ProfileSource::LocalPath(_)));
    }

    #[test]
    fn http_string_classifies_as_url() {
        let s = ProfileSource::classify("https://example.org/foo.csv").unwrap();
        assert!(matches!(s, ProfileSource::Url(_)));
    }

    #[test]
    fn bare_name_classifies_as_index_entry() {
        let s = ProfileSource::classify("destiny").unwrap();
        assert!(matches!(s, ProfileSource::IndexEntry(_)));
    }
}
