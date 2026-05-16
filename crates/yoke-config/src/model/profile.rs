use serde::{Deserialize, Serialize};

use crate::catalog::{Channel, SubProfileMode};
use crate::csv::raw::RawSection;
use crate::model::{Binding, PreferenceOverride, Preferences};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub top_line: TopLine,
    pub sub_profiles: Vec<SubProfile>,
    pub preferences: Option<Preferences>,
    pub infrared: Vec<RawSection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopLine {
    pub label: String,
    pub version: String,
    pub source: String,
    pub title: String,
    pub trailing_cells: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubProfile {
    pub header: SubProfileHeader,
    pub bindings: Vec<Binding>,
    pub overrides: Vec<PreferenceOverride>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubProfileHeader {
    pub profile_name: String,
    pub mode: SubProfileMode,
    pub sub_mode: String,
    pub channel: Channel,
    pub column_header_label: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_profile_constructs() {
        let p = Profile {
            top_line: TopLine {
                label: "QuadStick Configuration".into(),
                version: "Version 1.4".into(),
                source: String::new(),
                title: "Default".into(),
                trailing_cells: vec![],
            },
            sub_profiles: vec![],
            preferences: None,
            infrared: vec![],
        };
        assert_eq!(p.sub_profiles.len(), 0);
    }
}
