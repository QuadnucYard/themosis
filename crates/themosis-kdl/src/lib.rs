//! Derive-decoded KDL component-style input for Themosis.

#![forbid(unsafe_code)]

mod error;
mod parser;
mod raw;

pub use error::{ParseError, StructureError, StructureErrors};
pub use parser::parse;
