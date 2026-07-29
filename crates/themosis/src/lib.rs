//! Source discovery and end-to-end compilation for Themosis themes.

#![forbid(unsafe_code)]

mod loader;
mod paths;
mod provider;

pub use loader::{CompilationReport, LoadError, compile_theme, compile_theme_with_report};
pub use paths::InvalidSourcePath;
pub use provider::{
    FileSystemSourceProvider, MemorySourceProvider, SourceProvider, SourceReadError,
};
