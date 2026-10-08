//! Portable build planning and native builder assets for the Themosis Godot backend.

#![forbid(unsafe_code)]

mod backend;
mod errors;
pub mod runner;

#[cfg(test)]
mod tests;

pub use backend::{
    GodotBuildPlan, GodotItemKind, PlannedItem, PlannedStyle, PreparedValue, plan_theme,
};
pub use errors::{BackendError, BackendErrors};
pub use runner::{
    GodotVersion, LineColumn, RUNNER_SCHEMA_VERSION, RunnerDiagnostic, RunnerOperation,
    RunnerRequest, RunnerResponse, SourceSpan,
};

/// Portable GDScript builder for the headless CLI and engine-side integrations.
pub const NATIVE_THEME_BUILDER_GDSCRIPT: &str = include_str!("../assets/native_theme_builder.gd");

/// Headless entry point that validates requests and saves native theme resources.
pub const NATIVE_THEME_RUNNER_GDSCRIPT: &str = include_str!("../assets/native_theme_runner.gd");
