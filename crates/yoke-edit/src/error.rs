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
    #[error("sub-profile index {index} is out of range (profile has {len} sub-profiles)")]
    SubProfileIndexOutOfRange { index: usize, len: usize },
    #[error("cannot delete the last remaining sub-profile")]
    LastSubProfileDeletion,
    #[error("unknown modifier: {modifier:?}; did you mean: {suggestions:?}")]
    UnknownModifier {
        modifier: String,
        suggestions: Vec<String>,
    },
    #[error("modifier {keyword:?} does not accept the arguments in {modifier:?}")]
    InvalidModifierArguments { keyword: String, modifier: String },
    #[error(
        "input {input:?} with modifier {modifier:?} already maps to {output:?} in sub-profile {sub_profile}; use update-binding to change it"
    )]
    BindingExists {
        sub_profile: usize,
        input: String,
        modifier: String,
        output: String,
    },
    #[error(
        "no binding for input {input:?} in sub-profile {sub_profile} matches the given modifier/output"
    )]
    BindingNotFound { sub_profile: usize, input: String },
    #[error(
        "input {input:?} maps to {output:?} via multiple modifiers in sub-profile {sub_profile}; specify the modifier to disambiguate"
    )]
    AmbiguousBinding {
        sub_profile: usize,
        input: String,
        output: String,
    },
}

#[derive(Error, Debug, PartialEq, Eq)]
#[error("edit op {index} failed: {error}")]
pub struct ApplyError {
    pub index: usize,
    pub error: EditError,
}
