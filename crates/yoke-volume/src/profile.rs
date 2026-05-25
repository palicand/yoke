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

/// Coerce a free-form string into a token that `ProfileName::new` will always
/// accept.
///
/// The transform is lossy by design: callers reach for it after the user has
/// expressed intent via a bare name (`Half-Life: Alyx`, an URL basename with
/// percent-encoded spaces, a community-index title), so the result is a
/// best-effort filesystem-safe stem rather than a round-trip. The rules
/// stay aligned with `ProfileName::new` so the two cannot drift: anything
/// `ProfileName::new` rejects, this function replaces. If the entire input
/// collapses to nothing usable, the helper returns `"profile"` instead of
/// failing so the caller can still produce a default filename and surface
/// the collision via the existence check.
#[must_use]
pub fn sanitize_for_profile_name(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        let bad = ch.is_whitespace()
            || ch.is_control()
            || matches!(
                ch,
                '/' | '\\' | '\0' | ':' | '<' | '>' | '|' | '?' | '*' | '"'
            );
        if bad {
            out.push('_');
        } else {
            out.extend(ch.to_lowercase());
        }
    }
    // Collapse runs of '_' so `"a / b"` doesn't become `"a___b"`.
    let mut collapsed = String::with_capacity(out.len());
    let mut prev_underscore = false;
    for ch in out.chars() {
        if ch == '_' {
            if !prev_underscore {
                collapsed.push('_');
            }
            prev_underscore = true;
        } else {
            collapsed.push(ch);
            prev_underscore = false;
        }
    }
    let trimmed = collapsed.trim_matches('_');
    let truncated = if trimmed.len() <= 64 {
        trimmed
    } else {
        let mut cut = 0;
        for (i, ch) in trimmed.char_indices() {
            if i + ch.len_utf8() > 64 {
                break;
            }
            cut = i + ch.len_utf8();
        }
        &trimmed[..cut]
    };
    let final_stem = truncated.trim_end_matches('_');
    if final_stem.is_empty() {
        return "profile".into();
    }
    final_stem.to_string()
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
    fn sanitize_lowercases_and_replaces_whitespace() {
        assert_eq!(sanitize_for_profile_name("Destiny 2"), "destiny_2");
        assert_eq!(sanitize_for_profile_name("My\tProfile"), "my_profile");
    }

    #[test]
    fn sanitize_replaces_fat_illegal_chars() {
        assert_eq!(
            sanitize_for_profile_name("Half-Life: Alyx"),
            "half-life_alyx"
        );
        assert_eq!(sanitize_for_profile_name("a/b"), "a_b");
        assert_eq!(sanitize_for_profile_name("a\\b"), "a_b");
        assert_eq!(sanitize_for_profile_name("a<b>c"), "a_b_c");
        assert_eq!(sanitize_for_profile_name("a|b?c*d\"e"), "a_b_c_d_e");
    }

    #[test]
    fn sanitize_collapses_runs_and_trims() {
        assert_eq!(sanitize_for_profile_name("a   b"), "a_b");
        assert_eq!(sanitize_for_profile_name("  spaced  "), "spaced");
        assert_eq!(sanitize_for_profile_name("___"), "profile");
    }

    #[test]
    fn sanitize_falls_back_to_profile_when_empty() {
        assert_eq!(sanitize_for_profile_name(""), "profile");
        assert_eq!(sanitize_for_profile_name("///"), "profile");
    }

    #[test]
    fn sanitize_truncates_to_profile_name_byte_limit() {
        let long = "a".repeat(100);
        let s = sanitize_for_profile_name(&long);
        assert!(s.len() <= 64, "sanitized len {} exceeds 64", s.len());
        ProfileName::new(&s).expect("truncated sanitize output must satisfy ProfileName::new");
    }

    #[test]
    fn sanitize_truncates_on_utf8_boundary() {
        let raw = "é".repeat(40);
        let s = sanitize_for_profile_name(&raw);
        assert!(s.len() <= 64);
        assert!(s.is_char_boundary(s.len()));
        ProfileName::new(&s).unwrap();
    }

    #[test]
    fn sanitize_output_is_accepted_by_profile_name_new() {
        for raw in [
            "Destiny 2",
            "Half-Life: Alyx",
            "  spaced  ",
            "My\tProfile",
            "a/b\\c:d<e>f|g?h*i\"j",
        ] {
            let s = sanitize_for_profile_name(raw);
            ProfileName::new(&s).unwrap_or_else(|e| panic!("rejected {s:?}: {e:?}"));
        }
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
