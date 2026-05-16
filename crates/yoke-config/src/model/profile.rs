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
    // Total cell count emitted by the source file's top line; the writer
    // pads back to this width so explicit trailing commas survive round-trip.
    pub width: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubProfile {
    pub header: SubProfileHeader,
    // Bindings and overrides are stored in source order so a sub-profile whose
    // rows are interleaved round-trips byte-for-byte through the writer.
    pub rows: Vec<SubProfileRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubProfileRow {
    Binding(Binding),
    Override(PreferenceOverride),
}

impl SubProfile {
    pub fn bindings(&self) -> impl Iterator<Item = &Binding> {
        self.rows.iter().filter_map(|r| match r {
            SubProfileRow::Binding(b) => Some(b),
            SubProfileRow::Override(_) => None,
        })
    }

    pub fn overrides(&self) -> impl Iterator<Item = &PreferenceOverride> {
        self.rows.iter().filter_map(|r| match r {
            SubProfileRow::Override(o) => Some(o),
            SubProfileRow::Binding(_) => None,
        })
    }
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
                width: 4,
            },
            sub_profiles: vec![],
            preferences: None,
            infrared: vec![],
        };
        assert_eq!(p.sub_profiles.len(), 0);
    }
}
