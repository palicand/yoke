use crate::data::DataError;

const FIXTURE_CSV: &[u8] = include_bytes!("../../fixtures/default.csv");

/// wasm-side stand-in for `yoke_index::IndexEntry` (which is native-only).
#[derive(Debug, Clone)]
pub struct MockCommunityEntry {
    pub name: String,
    pub url: String,
}

/// wasm-side stand-in for `yoke_volume::state::MountState`. Only the variant the
/// status pill needs is modeled; the mock is always "present".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MockMountState {
    Present,
}

pub struct MockDataSource;

impl MockDataSource {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    fn parse_fixture() -> Result<yoke_config::ParseResult, DataError> {
        yoke_config::parse(FIXTURE_CSV).map_err(|e| DataError::Parse(e.to_string()))
    }
}

impl Default for MockDataSource {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::data::DataSource;

    #[test]
    fn fixture_parses_into_a_profile_with_subprofiles() {
        let result = MockDataSource::parse_fixture().expect("fixture must parse");
        assert!(
            !result.model.sub_profiles.is_empty(),
            "fixture has at least one sub-profile"
        );
    }

    #[test]
    fn lists_at_least_one_device_profile() {
        let data = MockDataSource::new();
        let list = data.list_device_profiles().unwrap();
        assert!(!list.is_empty());
    }

    #[test]
    fn lists_at_least_one_community_entry() {
        let data = MockDataSource::new();
        let list = data.list_community().unwrap();
        assert!(!list.is_empty());
    }

    #[test]
    fn reads_file_profile_ignoring_path() {
        let data = MockDataSource::new();
        let result = data.read_file_profile(Path::new("ignored.csv")).unwrap();
        assert!(!result.model.sub_profiles.is_empty());
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::collections::BTreeMap;
    use std::path::Path;

    use url::Url;
    use yoke_index::IndexEntry;
    use yoke_volume::ProfileName;
    use yoke_volume::state::{MountState, VidPid};

    use crate::data::mock::MockDataSource;
    use crate::data::{DataError, DataSource, ProfileEntryView};

    impl DataSource for MockDataSource {
        fn volume_state(&self) -> MountState {
            MountState::Present {
                mount_point: std::path::PathBuf::from("/Volumes/QUADSTICK"),
                vid_pid: VidPid {
                    vendor: 0x16D0,
                    product: 0x092B,
                },
                label: "QUADSTICK".into(),
            }
        }

        fn list_device_profiles(&self) -> Result<Vec<ProfileEntryView>, DataError> {
            Ok(vec![ProfileEntryView {
                name: ProfileName::new("default").unwrap(),
                label: "default.csv".into(),
            }])
        }

        fn read_device_profile(
            &self,
            _name: &ProfileName,
        ) -> Result<yoke_config::ParseResult, DataError> {
            Self::parse_fixture()
        }

        fn read_file_profile(&self, _path: &Path) -> Result<yoke_config::ParseResult, DataError> {
            Self::parse_fixture()
        }

        fn write_file_profile(&self, _path: &Path, _bytes: &[u8]) -> Result<(), DataError> {
            Ok(())
        }

        fn write_device_profile(
            &self,
            _name: &ProfileName,
            _bytes: &[u8],
        ) -> Result<(), DataError> {
            Ok(())
        }

        fn list_community(&self) -> Result<Vec<IndexEntry>, DataError> {
            Ok(vec![IndexEntry {
                name: "Destiny 2 (sample)".into(),
                csv_url: Url::parse("https://example.org/d2.csv").unwrap(),
                fields: BTreeMap::new(),
            }])
        }

        fn fetch_community(
            &self,
            _entry: &IndexEntry,
        ) -> Result<yoke_config::ParseResult, DataError> {
            Self::parse_fixture()
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use std::path::Path;

    use crate::data::mock::{MockCommunityEntry, MockDataSource, MockMountState};
    use crate::data::{DataError, DataSource, ProfileEntryView};

    impl DataSource for MockDataSource {
        fn volume_state(&self) -> MockMountState {
            MockMountState::Present
        }

        fn list_device_profiles(&self) -> Result<Vec<ProfileEntryView>, DataError> {
            Ok(vec![ProfileEntryView {
                name: "default".into(),
                label: "default.csv".into(),
            }])
        }

        fn read_device_profile(
            &self,
            _name: &String,
        ) -> Result<yoke_config::ParseResult, DataError> {
            Self::parse_fixture()
        }

        fn read_file_profile(&self, _path: &Path) -> Result<yoke_config::ParseResult, DataError> {
            Self::parse_fixture()
        }

        fn write_file_profile(&self, _path: &Path, _bytes: &[u8]) -> Result<(), DataError> {
            Ok(())
        }

        fn write_device_profile(&self, _name: &String, _bytes: &[u8]) -> Result<(), DataError> {
            Ok(())
        }

        fn list_community(&self) -> Result<Vec<MockCommunityEntry>, DataError> {
            Ok(vec![MockCommunityEntry {
                name: "Destiny 2 (sample)".into(),
                url: "https://example.org/d2.csv".into(),
            }])
        }

        fn fetch_community(
            &self,
            _entry: &MockCommunityEntry,
        ) -> Result<yoke_config::ParseResult, DataError> {
            Self::parse_fixture()
        }
    }
}
