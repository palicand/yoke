#![forbid(unsafe_code)]

pub mod apply;
pub mod error;
pub mod op;
pub mod suggest;

pub use apply::apply;
pub use error::{ApplyError, EditError};
pub use op::{EditOp, PreferenceValue};
