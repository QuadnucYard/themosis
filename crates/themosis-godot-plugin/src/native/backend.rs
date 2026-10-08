//! Native Godot theme construction.

mod apply;
mod diagnostic;

use godot::{classes::Theme, prelude::*};
use themosis_core::CompiledTheme;
use themosis_godot::plan_theme;

pub use self::diagnostic::ThemeBuildError;

/// Builds a native Godot theme from canonical compiler output.
///
/// `themosis-godot` validates the compiled theme and normalizes it into a
/// portable build plan whose items carry candidate native categories. This
/// function resolves those candidates against the running engine's default theme
/// and control-type chain, then constructs the resource directly in Rust.
pub fn build_theme(compiled: &CompiledTheme) -> Result<Gd<Theme>, ThemeBuildError> {
    let plan = plan_theme(compiled)?;
    apply::build_native_theme(&plan).map_err(ThemeBuildError::Native)
}
