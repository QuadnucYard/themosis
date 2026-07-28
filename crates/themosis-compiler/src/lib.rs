//! Pure semantic compilation for Themosis themes.

#![forbid(unsafe_code)]

mod diagnostics;
mod error;
mod styles;
mod tokens;

pub use diagnostics::{CompileErrors, DiagnosticLabel, DiagnosticMetadata};
pub use error::CompileError;
pub use styles::compile_styles;
pub use tokens::resolve_tokens;
