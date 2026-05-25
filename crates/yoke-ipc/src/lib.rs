#![forbid(unsafe_code)]

//! Wire DTOs for the Tauri <-> Leptos IPC surface.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub use yoke_config::model::Profile;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceProfileEntry {
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommunityEntry {
    pub name: String,
    pub url: String,
    pub fields: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind")]
pub enum VolumePresence {
    Absent,
    DeviceVisibleNoVolume { mode_hint: Option<String> },
    Present { label: String, mount_point: PathBuf },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, thiserror::Error)]
#[serde(tag = "kind", content = "detail")]
pub enum BackendError {
    #[error("backend not initialized: {0}")]
    NotInitialized(String),
    #[error("volume not present")]
    VolumeNotPresent,
    #[error("io: {0}")]
    Io(String),
    #[error("parse: {0}")]
    Parse(String),
    #[error("network: {0}")]
    Network(String),
    #[error("not found: {0}")]
    NotFound(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_presence_round_trips() {
        let v = VolumePresence::Present {
            label: "Quad Stick".into(),
            mount_point: PathBuf::from("/Volumes/Quad Stick"),
        };
        let json = serde_json::to_string(&v).unwrap();
        let back: VolumePresence = serde_json::from_str(&json).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn backend_error_round_trips() {
        let e = BackendError::Parse("bad CSV".into());
        let json = serde_json::to_string(&e).unwrap();
        let back: BackendError = serde_json::from_str(&json).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn community_entry_round_trips() {
        let mut fields = std::collections::BTreeMap::new();
        fields.insert("variant".to_string(), "FPS".to_string());
        let c = CommunityEntry {
            name: "Alice's FPS preset".into(),
            url: "https://example.invalid/a.csv".into(),
            fields,
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: CommunityEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }
}
