use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PreferenceValue {
    Bool(bool),
    Number(i64),
    Text(String),
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
}
