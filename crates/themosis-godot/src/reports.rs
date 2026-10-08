//! Portable outcomes for compile, validate, and materialize operations.
//!
//! The GDExtension addon runs these operations against a live engine; this
//! module owns their result shape so the editor dock, the headless runners, and
//! the tests share one Rust representation without Godot dictionaries.

use crate::runner::RunnerDiagnostic;

/// Operation that produced an [`OperationOutcome`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operation {
    /// Compiling a root through Godot's import pipeline.
    Import,
    /// Compiling a root without saving an artifact.
    Validate,
    /// Compiling and saving a visible `.tres` artifact.
    Materialize,
}

impl Operation {
    /// Returns the label the editor dock uses for this operation.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Import => "Import",
            Self::Validate => "Validation",
            Self::Materialize => "Materialization",
        }
    }
}

/// Status of one operation outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutcomeStatus {
    /// The root compiled and, when requested, its artifact was written.
    Success,
    /// The root compiled and no artifact was requested.
    Validated,
    /// The operation failed; `error` and `diagnostics` describe why.
    Failure,
}

impl OutcomeStatus {
    /// Returns whether the operation completed successfully.
    #[must_use]
    pub fn is_ok(self) -> bool {
        matches!(self, Self::Success | Self::Validated)
    }
}

/// Result of one compile, validate, or materialize operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationOutcome {
    /// Operation that produced this outcome.
    pub operation: Operation,
    /// Profile name, when the operation ran a configured profile.
    pub profile: String,
    /// `res://` theme root the operation compiled.
    pub source: String,
    /// `res://` artifact the operation saved, when it saved one.
    pub output: String,
    /// Whether the operation succeeded.
    pub status: OutcomeStatus,
    /// Failure message; empty on success.
    pub error: String,
    /// Structured failures; empty on success.
    pub diagnostics: Vec<RunnerDiagnostic>,
    /// Every source and resource the compilation depends on.
    pub dependencies: Vec<String>,
    /// Milliseconds the operation took.
    pub elapsed_ms: i64,
}

impl OperationOutcome {
    /// Returns whether the operation succeeded.
    #[must_use]
    pub fn ok(&self) -> bool {
        self.status.is_ok()
    }

    /// Creates a failure outcome without dependencies or timing.
    #[must_use]
    pub fn failure(
        operation: Operation,
        profile: &str,
        source: &str,
        output: &str,
        error: impl Into<String>,
        diagnostics: Vec<RunnerDiagnostic>,
    ) -> Self {
        Self {
            operation,
            profile: profile.to_owned(),
            source: source.to_owned(),
            output: output.to_owned(),
            status: OutcomeStatus::Failure,
            error: error.into(),
            diagnostics,
            dependencies: Vec::new(),
            elapsed_ms: 0,
        }
    }
}

/// Returns the last path segment of a `res://` path.
#[must_use]
pub fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or_default()
}

/// Returns the last path segment of a `res://` path without its extension.
#[must_use]
pub fn file_stem(path: &str) -> &str {
    match file_name(path).rfind('.') {
        Some(position) => &file_name(path)[..position],
        None => file_name(path),
    }
}
