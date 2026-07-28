//! DTCG-compatible JSON token input for Themosis.

#![forbid(unsafe_code)]

mod error;
mod parser;

pub use error::{ParseError, ParseErrors};
pub use parser::parse;
