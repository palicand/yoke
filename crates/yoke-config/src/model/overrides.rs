use serde::{Deserialize, Serialize};

use crate::catalog::PreferenceKey;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreferenceOverride {
    pub key: PreferenceKey,
    pub value: String,
    pub comment: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_serde_round_trips() {
        let o = PreferenceOverride {
            key: PreferenceKey::from_csv("joystick_dead_zone_shape").unwrap(),
            value: "1".into(),
            comment: None,
        };
        let json = serde_json::to_string(&o).unwrap();
        let back: PreferenceOverride = serde_json::from_str(&json).unwrap();
        assert_eq!(o, back);
    }
}
