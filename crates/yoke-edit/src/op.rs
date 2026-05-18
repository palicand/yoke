use serde::{Deserialize, Serialize};
use yoke_config::catalog::{Channel, SubProfileMode};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PreferenceValue {
    Bool(bool),
    Number(i64),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub enum EditOp {
    SetTitle {
        title: String,
    },
    SetPreference {
        key: String,
        value: PreferenceValue,
    },
    UnsetPreference {
        key: String,
    },
    SetBinding {
        sub_profile: String,
        input: String,
        output: String,
    },
    ClearBinding {
        sub_profile: String,
        input: String,
    },
    SetOverride {
        sub_profile: String,
        key: String,
        value: PreferenceValue,
    },
    UnsetOverride {
        sub_profile: String,
        key: String,
    },
    AddSubProfile {
        name: String,
        mode: SubProfileMode,
        sub_mode: String,
        channel: Channel,
    },
    DeleteSubProfile {
        name: String,
    },
    RenameSubProfile {
        from: String,
        to: String,
    },
    CloneSubProfile {
        from: String,
        to: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_round_trips_as_bare_int() {
        let v = PreferenceValue::Number(35);
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, "35");
        let back: PreferenceValue = serde_json::from_str(&json).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn bool_round_trips_as_bare_bool() {
        let v = PreferenceValue::Bool(true);
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, "true");
        let back: PreferenceValue = serde_json::from_str(&json).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn text_round_trips_as_bare_string() {
        let v = PreferenceValue::Text("foo".into());
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, "\"foo\"");
        let back: PreferenceValue = serde_json::from_str(&json).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn edit_op_set_preference_round_trips_kebab_case_tag() {
        let op = EditOp::SetPreference {
            key: "volume".into(),
            value: PreferenceValue::Number(55),
        };
        let json = serde_json::to_value(&op).unwrap();
        assert_eq!(json["op"], "set-preference");
        assert_eq!(json["key"], "volume");
        assert_eq!(json["value"], 55);
        let back: EditOp = serde_json::from_value(json).unwrap();
        assert_eq!(op, back);
    }

    #[test]
    fn edit_op_rename_sub_profile_round_trips() {
        let op = EditOp::RenameSubProfile {
            from: "Main".into(),
            to: "Cougar".into(),
        };
        let json = serde_json::to_value(&op).unwrap();
        assert_eq!(json["op"], "rename-sub-profile");
        let back: EditOp = serde_json::from_value(json).unwrap();
        assert_eq!(op, back);
    }
}
