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
        if raw.starts_with("http://") || raw.starts_with("https://") {
            let url = Url::parse(raw).map_err(|e| IndexError::InvalidUrl(e.to_string()))?;
            return Ok(Self::Url(url));
        }
        // Path-like syntax wins over IndexEntry so a community-index name like
        // "Destiny 2" cannot be shadowed by a cwd file of the same name. Bare
        // tokens always fall through to IndexEntry; the caller can pass `./foo`
        // to force local-file semantics.
        if is_path_like(raw) {
            return Ok(Self::LocalPath(PathBuf::from(raw)));
        }
        Ok(Self::IndexEntry(raw.to_string()))
    }
}

fn is_path_like(raw: &str) -> bool {
    PathBuf::from(raw).is_absolute()
        || raw.starts_with("./")
        || raw.starts_with("../")
        || raw.starts_with(r".\")
        || raw.starts_with(r"..\")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn absolute_path_classifies_as_local() {
        let f = NamedTempFile::new().unwrap();
        let s = ProfileSource::classify(f.path().to_str().unwrap()).unwrap();
        assert!(matches!(s, ProfileSource::LocalPath(_)));
    }

    #[test]
    fn dot_slash_path_classifies_as_local_even_if_missing() {
        let s = ProfileSource::classify("./does-not-exist.csv").unwrap();
        assert!(matches!(s, ProfileSource::LocalPath(_)));
    }

    #[test]
    fn parent_relative_path_classifies_as_local() {
        let s = ProfileSource::classify("../foo.csv").unwrap();
        assert!(matches!(s, ProfileSource::LocalPath(_)));
    }

    #[test]
    fn embedded_slash_without_prefix_classifies_as_index_entry() {
        let s = ProfileSource::classify("sub/dir/profile.csv").unwrap();
        assert!(matches!(s, ProfileSource::IndexEntry(_)));
    }

    #[test]
    fn index_entry_with_slash_in_name() {
        let s = ProfileSource::classify("Star Wars: Jedi/Survivor").unwrap();
        assert!(matches!(s, ProfileSource::IndexEntry(_)));
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

    #[test]
    fn bare_name_with_spaces_classifies_as_index_entry() {
        let s = ProfileSource::classify("Destiny 2").unwrap();
        assert!(matches!(s, ProfileSource::IndexEntry(_)));
    }
}
