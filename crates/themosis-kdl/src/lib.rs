//! KDL component-style input for Themosis.

#![forbid(unsafe_code)]

mod decode;
mod error;
mod parser;

pub use error::{ParseError, ParseErrors, StructureError, SyntaxError};
pub use parser::parse;
