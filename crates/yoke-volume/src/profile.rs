use crate::error::VolumeError;
use std::time::SystemTime;

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ProfileName(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ProfileKind {
    Default,
    Prefs,
    Game,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ProfileEntry {
    pub name: ProfileName,
    pub kind: ProfileKind,
    pub byte_len: u64,
    pub modified: SystemTime,
}

impl ProfileName {
    pub fn new(raw: &str) -> Result<Self, VolumeError> {
        let stem =
            if raw.len() >= 4 && raw.as_bytes()[raw.len() - 4..].eq_ignore_ascii_case(b".csv") {
                &raw[..raw.len() - 4]
            } else {
                raw
            };
        if stem.is_empty() {
            return Err(VolumeError::InvalidProfileName(raw.to_string()));
        }
        if stem.len() > 64 {
            return Err(VolumeError::InvalidProfileName(raw.to_string()));
        }
        for ch in stem.chars() {
            let illegal = matches!(
                ch,
                '/' | '\\' | '\0' | ':' | '<' | '>' | '|' | '?' | '*' | '"'
            ) || ch.is_control();
            if illegal {
                return Err(VolumeError::InvalidProfileName(raw.to_string()));
            }
        }
        Ok(Self(format!("{stem}.csv")))
    }

    #[must_use]
    pub fn as_filename(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn stem(&self) -> &str {
        self.0.strip_suffix(".csv").unwrap_or(&self.0)
    }

    #[must_use]
    pub fn kind(&self) -> ProfileKind {
        match self.0.to_ascii_lowercase().as_str() {
            "default.csv" => ProfileKind::Default,
            "prefs.csv" => ProfileKind::Prefs,
            _ => ProfileKind::Game,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_plain_name_and_appends_csv() {
        let n = ProfileName::new("destiny").unwrap();
        assert_eq!(n.as_filename(), "destiny.csv");
        assert_eq!(n.stem(), "destiny");
    }

    #[test]
    fn accepts_name_with_csv_suffix() {
        let n = ProfileName::new("destiny.csv").unwrap();
        assert_eq!(n.as_filename(), "destiny.csv");
    }

    #[test]
    fn strips_csv_suffix_case_insensitively() {
        let n = ProfileName::new("DEFAULT.CSV").unwrap();
        assert_eq!(n.as_filename(), "DEFAULT.csv");
        assert_eq!(n.kind(), ProfileKind::Default);
        let n = ProfileName::new("Destiny.Csv").unwrap();
        assert_eq!(n.as_filename(), "Destiny.csv");
    }

    #[test]
    fn rejects_empty_stem() {
        assert!(matches!(
            ProfileName::new(""),
            Err(VolumeError::InvalidProfileName(_))
        ));
        assert!(matches!(
            ProfileName::new(".csv"),
            Err(VolumeError::InvalidProfileName(_))
        ));
    }

    #[test]
    fn rejects_path_separators() {
        for bad in &["foo/bar", "foo\\bar", "../foo", "foo\0"] {
            assert!(
                matches!(
                    ProfileName::new(bad),
                    Err(VolumeError::InvalidProfileName(_))
                ),
                "should reject {bad:?}"
            );
        }
    }

    #[test]
    fn rejects_fat_illegal_chars() {
        for bad in &["a:b", "a<b", "a>b", "a|b", "a?b", "a*b", "a\"b"] {
            assert!(
                matches!(
                    ProfileName::new(bad),
                    Err(VolumeError::InvalidProfileName(_))
                ),
                "should reject {bad:?}"
            );
        }
    }

    #[test]
    fn rejects_non_printable() {
        assert!(matches!(
            ProfileName::new("foo\nbar"),
            Err(VolumeError::InvalidProfileName(_))
        ));
        assert!(matches!(
            ProfileName::new("foo\tbar"),
            Err(VolumeError::InvalidProfileName(_))
        ));
    }

    #[test]
    fn rejects_overlong_stem() {
        let long = "a".repeat(65);
        assert!(matches!(
            ProfileName::new(&long),
            Err(VolumeError::InvalidProfileName(_))
        ));
        let max = "a".repeat(64);
        assert!(ProfileName::new(&max).is_ok());
    }

    #[test]
    fn kind_classification() {
        assert_eq!(
            ProfileName::new("default").unwrap().kind(),
            ProfileKind::Default
        );
        assert_eq!(
            ProfileName::new("DEFAULT.csv").unwrap().kind(),
            ProfileKind::Default
        );
        assert_eq!(
            ProfileName::new("prefs").unwrap().kind(),
            ProfileKind::Prefs
        );
        assert_eq!(
            ProfileName::new("destiny").unwrap().kind(),
            ProfileKind::Game
        );
    }
}
