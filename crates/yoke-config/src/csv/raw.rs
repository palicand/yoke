use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawCsv {
    pub top_line: Vec<String>,
    /// Blank lines between the top line and `sections[0]`.
    /// `blank_runs[i]` covers the blanks AFTER `sections[i]`.
    #[serde(default)]
    pub leading_blanks: usize,
    pub sections: Vec<RawSection>,
    pub blank_runs: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawSection {
    pub rows: Vec<RawRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawRow {
    pub cells: Vec<String>,
}

impl RawCsv {
    pub fn from_bytes(input: &[u8]) -> Result<Self, crate::error::ParseError> {
        crate::csv::parse::read_raw(input)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        crate::csv::write::write_raw(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = b"QuadStick Configuration,Version 1.4,abc,Test\r\n\
Profile Name,,Mouse,\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mouse_left,normal,left,\r\n\
\r\n\
Preferences,\r\n\
,,,,\r\n\
Preference,Value,Units,Description,\r\n\
volume,40,,,\r\n";

    #[test]
    fn raw_round_trip_is_byte_identical() {
        let raw = RawCsv::from_bytes(SAMPLE).expect("parse must succeed");
        let bytes = raw.to_bytes();
        pretty_assertions::assert_eq!(
            std::str::from_utf8(&bytes).unwrap(),
            std::str::from_utf8(SAMPLE).unwrap()
        );
    }

    #[test]
    fn raw_preserves_blank_separators() {
        let raw = RawCsv::from_bytes(SAMPLE).unwrap();
        assert_eq!(raw.sections.len(), 2);
    }

    #[test]
    fn raw_preserves_trailing_commas() {
        let raw = RawCsv::from_bytes(SAMPLE).unwrap();
        assert_eq!(raw.sections[0].rows[0].cells.len(), 4);
    }

    const WITH_LEADING_BLANK: &[u8] = b"QuadStick Configuration,Version 1.4,abc,Test\r\n\
\r\n\
Profile Name,,Mouse,\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mouse_left,normal,left,\r\n\
\r\n";

    #[test]
    fn raw_preserves_blank_run_before_first_section() {
        let raw = RawCsv::from_bytes(WITH_LEADING_BLANK).expect("parse");
        assert_eq!(raw.leading_blanks, 1);
        let bytes = raw.to_bytes();
        pretty_assertions::assert_eq!(
            std::str::from_utf8(&bytes).unwrap(),
            std::str::from_utf8(WITH_LEADING_BLANK).unwrap()
        );
    }

    #[test]
    fn rejects_non_utf8() {
        let bad: &[u8] = &[0xFF, 0xFE, b'\r', b'\n'];
        match RawCsv::from_bytes(bad) {
            Err(crate::error::ParseError::Encoding) => (),
            other => panic!("expected Encoding, got {other:?}"),
        }
    }
}
