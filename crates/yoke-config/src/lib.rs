#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]

pub mod catalog;
pub mod csv;
pub mod error;
pub mod model;

pub use crate::csv::parse::{ParseResult, parse};
pub use crate::csv::write::write;
pub use error::{ParseError, Warning, WriteError};
