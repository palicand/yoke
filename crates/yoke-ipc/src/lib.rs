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
    fn volume_presence_round_trips_every_variant() {
        let cases = vec![
            VolumePresence::Absent,
            VolumePresence::DeviceVisibleNoVolume { mode_hint: None },
            VolumePresence::DeviceVisibleNoVolume {
                mode_hint: Some("DS3".into()),
            },
            VolumePresence::Present {
                label: "Quad Stick".into(),
                mount_point: PathBuf::from("/Volumes/Quad Stick"),
            },
        ];
        for v in cases {
            let json = serde_json::to_string(&v).unwrap();
            let back: VolumePresence = serde_json::from_str(&json).unwrap();
            assert_eq!(v, back, "round-trip mismatch for {v:?}");
        }
    }

    #[test]
    fn volume_presence_wire_shape() {
        assert_eq!(
            serde_json::to_string(&VolumePresence::Absent).unwrap(),
            r#"{"kind":"Absent"}"#,
        );
        assert_eq!(
            serde_json::to_string(&VolumePresence::DeviceVisibleNoVolume { mode_hint: None })
                .unwrap(),
            r#"{"kind":"DeviceVisibleNoVolume","mode_hint":null}"#,
        );
        assert_eq!(
            serde_json::to_string(&VolumePresence::DeviceVisibleNoVolume {
                mode_hint: Some("DS3".into()),
            })
            .unwrap(),
            r#"{"kind":"DeviceVisibleNoVolume","mode_hint":"DS3"}"#,
        );
        assert_eq!(
            serde_json::to_string(&VolumePresence::Present {
                label: "Quad Stick".into(),
                mount_point: PathBuf::from("/Volumes/Quad Stick"),
            })
            .unwrap(),
            r#"{"kind":"Present","label":"Quad Stick","mount_point":"/Volumes/Quad Stick"}"#,
        );
    }

    #[test]
    fn backend_error_round_trips_every_variant() {
        let cases = vec![
            BackendError::NotInitialized("not ready".into()),
            BackendError::VolumeNotPresent,
            BackendError::Io("disk fell off".into()),
            BackendError::Parse("bad CSV".into()),
            BackendError::Network("timeout".into()),
            BackendError::NotFound("missing.csv".into()),
        ];
        for e in cases {
            let json = serde_json::to_string(&e).unwrap();
            let back: BackendError = serde_json::from_str(&json).unwrap();
            assert_eq!(e, back, "round-trip mismatch for {e:?}");
        }
    }

    #[test]
    fn backend_error_wire_shape() {
        assert_eq!(
            serde_json::to_string(&BackendError::NotInitialized("x".into())).unwrap(),
            r#"{"kind":"NotInitialized","detail":"x"}"#,
        );
        assert_eq!(
            serde_json::to_string(&BackendError::VolumeNotPresent).unwrap(),
            r#"{"kind":"VolumeNotPresent"}"#,
        );
        assert_eq!(
            serde_json::to_string(&BackendError::Io("disk".into())).unwrap(),
            r#"{"kind":"Io","detail":"disk"}"#,
        );
        assert_eq!(
            serde_json::to_string(&BackendError::Parse("p".into())).unwrap(),
            r#"{"kind":"Parse","detail":"p"}"#,
        );
        assert_eq!(
            serde_json::to_string(&BackendError::Network("n".into())).unwrap(),
            r#"{"kind":"Network","detail":"n"}"#,
        );
        assert_eq!(
            serde_json::to_string(&BackendError::NotFound("nf".into())).unwrap(),
            r#"{"kind":"NotFound","detail":"nf"}"#,
        );
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
