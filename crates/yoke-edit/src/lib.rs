#![forbid(unsafe_code)]

pub mod error;
pub mod op;

pub use error::{ApplyError, EditError};
pub use op::{EditOp, PreferenceValue};
