use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum EditError {
    #[error("unknown input: {input:?}; did you mean: {suggestions:?}")]
    UnknownInput {
        input: String,
        suggestions: Vec<String>,
    },
    #[error("unknown output: {output:?}; did you mean: {suggestions:?}")]
    UnknownOutput {
        output: String,
        suggestions: Vec<String>,
    },
    #[error("unknown preference key: {key:?}; did you mean: {suggestions:?}")]
    UnknownPreference {
        key: String,
        suggestions: Vec<String>,
    },
    #[error("preference {key}: value {value:?} is not a valid {expected_type}")]
    InvalidPreferenceValue {
        key: String,
        value: String,
        expected_type: String,
    },
    #[error("sub-profile not found: {name:?}")]
    SubProfileNotFound { name: String },
    #[error("sub-profile already exists: {name:?}")]
    SubProfileExists { name: String },
    #[error("cannot delete the last remaining sub-profile")]
    LastSubProfileDeletion,
    #[error("unknown modifier: {modifier:?}; did you mean: {suggestions:?}")]
    UnknownModifier {
        modifier: String,
        suggestions: Vec<String>,
    },
    #[error("no binding for input {input:?} in sub-profile {sub_profile:?}; set its output first")]
    NoBindingForInput { sub_profile: String, input: String },
}

#[derive(Error, Debug, PartialEq, Eq)]
#[error("edit op {index} failed: {error}")]
pub struct ApplyError {
    pub index: usize,
    pub error: EditError,
}
