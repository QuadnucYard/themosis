//! Native Godot resource construction and GDExtension integration for Themosis.

mod backend;
mod extension;
mod provider;

#[cfg(debug_assertions)]
mod debug;

pub use backend::{NativeDiagnostic, NativeDiagnostics, ThemeBuildError, build_theme};
pub use extension::ThemosisThemeGenerator;
pub use provider::GodotSourceProvider;
