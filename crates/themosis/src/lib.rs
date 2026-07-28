//! Source discovery and end-to-end compilation for Themosis themes.

#![forbid(unsafe_code)]

mod loader;
mod paths;
mod provider;

pub use loader::{LoadError, compile_theme};
pub use paths::InvalidSourcePath;
pub use provider::{
    FileSystemSourceProvider, MemorySourceProvider, SourceProvider, SourceReadError,
};
