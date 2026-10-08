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
