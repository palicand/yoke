use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("input is not valid UTF-8")]
    Encoding,
    #[error("missing top-line header (first row of the file)")]
    MissingTopLine,
    #[error("malformed section header at line {line}")]
    MalformedSectionHeader { line: usize },
    #[error("unclosed quote at line {line}")]
    UnclosedQuote { line: usize },
    #[error("CSV tokenization failed: {0}")]
    Csv(#[from] csv::Error),
}

#[derive(Debug, Error)]
pub enum WriteError {
    #[error("invariant violated: {0}")]
    InvariantViolation(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Warning {
    UnknownOutput {
        id: String,
        line: usize,
    },
    UnknownInput {
        id: String,
        line: usize,
    },
    UnknownModifier {
        name: String,
        line: usize,
    },
    UnknownPreference {
        id: String,
        line: usize,
    },
    PreferenceOutOfRange {
        key: String,
        value: String,
        expected: String,
        line: usize,
    },
    DataAfterTerminator {
        line: usize,
    },
    DuplicateBinding {
        input: String,
        line: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_error_carries_line_context() {
        let err = ParseError::MalformedSectionHeader { line: 42 };
        let display = format!("{err}");
        assert!(display.contains("42"));
    }

    #[test]
    fn warning_is_clonable_and_eq() {
        let w = Warning::UnknownOutput {
            id: "mystery_output".into(),
            line: 7,
        };
        let copy = w.clone();
        assert_eq!(w, copy);
    }

    #[test]
    fn write_error_invariant_violation_holds_message() {
        let err = WriteError::InvariantViolation("bad mode combo".into());
        assert!(format!("{err}").contains("bad mode combo"));
    }
}
