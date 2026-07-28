//! Pure semantic compilation for Themosis themes.

#![forbid(unsafe_code)]

mod error;
mod tokens;

pub use error::{CompileError, CompileErrors};
pub use tokens::resolve_tokens;
