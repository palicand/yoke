use std::path::PathBuf;

use yoke_config::model::Profile;

#[cfg(not(target_arch = "wasm32"))]
use {url::Url, yoke_index::IndexEntry, yoke_volume::ProfileName};

#[cfg(target_arch = "wasm32")]
mod wasm_shims {
    pub type Url = String;
    pub type IndexEntry = crate::data::mock::MockCommunityEntry;
    pub type ProfileName = String;
}
#[cfg(target_arch = "wasm32")]
use wasm_shims::{IndexEntry, ProfileName, Url};

#[derive(Debug, Clone)]
pub enum ProfileSource {
    Device(ProfileName),
    File(PathBuf),
    Community { name: String, url: Url },
}

#[derive(Debug, Clone)]
pub struct OpenProfile {
    pub source: ProfileSource,
    pub profile: Profile,
}

#[derive(Debug, Clone)]
pub enum CommunityLoad {
    Loading,
    Loaded(Vec<IndexEntry>),
    Failed(String),
}

impl ProfileSource {
    /// Human-readable breadcrumb shown in the editor header.
    #[must_use]
    pub fn breadcrumb(&self) -> String {
        match self {
            Self::Device(name) => {
                #[cfg(not(target_arch = "wasm32"))]
                let file = name.as_filename().to_owned();
                #[cfg(target_arch = "wasm32")]
                let file = name.clone();
                format!("QuadStick / {file}")
            }
            Self::File(path) => {
                let stem = path.file_name().map_or_else(
                    || path.to_string_lossy().into_owned(),
                    |s| s.to_string_lossy().into_owned(),
                );
                format!("Local file / {stem}")
            }
            Self::Community { name, .. } => format!("Community / {name}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_breadcrumb_uses_filename() {
        let src = ProfileSource::Device(ProfileName::new("destiny").unwrap());
        assert_eq!(src.breadcrumb(), "QuadStick / destiny.csv");
    }

    #[test]
    fn file_breadcrumb_uses_basename() {
        let src = ProfileSource::File(PathBuf::from("/tmp/games/portal2.csv"));
        assert_eq!(src.breadcrumb(), "Local file / portal2.csv");
    }

    #[test]
    fn community_breadcrumb_uses_name() {
        let src = ProfileSource::Community {
            name: "Destiny 2".into(),
            url: Url::parse("https://example.org/d2.csv").unwrap(),
        };
        assert_eq!(src.breadcrumb(), "Community / Destiny 2");
    }
}
