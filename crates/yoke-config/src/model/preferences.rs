use serde::{Deserialize, Serialize};

use crate::catalog::PreferenceKey;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preferences {
    // Vec preserves the source-file row order required for byte-identical round-trips.
    pub entries: Vec<(String, PreferenceEntry)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreferenceEntry {
    pub key: PreferenceKey,
    pub value: String,
    pub units: String,
    pub description: String,
    pub comment: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::KnownPreference;

    #[test]
    fn preferences_lookup_by_id() {
        let mut prefs = Preferences::default();
        prefs.entries.push((
            "volume".into(),
            PreferenceEntry {
                key: PreferenceKey::Known(KnownPreference::Volume),
                value: "55".into(),
                units: String::new(),
                description: String::new(),
                comment: None,
            },
        ));
        let found = prefs
            .entries
            .iter()
            .find(|(k, _)| k == "volume")
            .map(|(_, e)| &*e.value);
        assert_eq!(found, Some("55"));
    }
}
