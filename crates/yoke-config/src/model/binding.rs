use serde::{Deserialize, Serialize};

use crate::catalog::{Input, Modifier, Output};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    pub output: Output,
    pub modifier: Modifier,
    pub input: Option<Input>,
    pub comment: Option<String>,
}

impl Binding {
    pub const fn new(output: Output, modifier: Modifier, input: Option<Input>) -> Self {
        Self {
            output,
            modifier,
            input,
            comment: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_serde_round_trips() {
        let b = Binding::new(
            Output::Touch,
            Modifier::Normal,
            Some(Input::Lip { soft: false }),
        );
        let json = serde_json::to_string(&b).unwrap();
        let back: Binding = serde_json::from_str(&json).unwrap();
        assert_eq!(b, back);
    }
}
