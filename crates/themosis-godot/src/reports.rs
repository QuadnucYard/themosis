//! Portable outcomes for compile, validate, and materialize operations.
//!
//! The GDExtension addon runs these operations against a live engine; this
//! module owns their result shape so the editor dock, the headless runners, and
//! the tests share one Rust representation without Godot dictionaries.

use std::collections::BTreeMap;

use crate::{profiles, runner::RunnerDiagnostic};

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

/// One destination claimed by more than one source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DestinationCollision {
    /// `res://` artifact both sources would write.
    pub output: String,
    /// Sources that resolve to the same artifact, in discovery order.
    pub sources: Vec<String>,
}

/// Result of a bulk build or materialization.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BatchOutcome {
    /// One outcome per attempted source.
    pub results: Vec<OperationOutcome>,
    /// Destinations rejected before anything was written.
    pub collisions: Vec<DestinationCollision>,
    /// Batch-level failure; empty when the batch succeeded.
    pub error: String,
}

impl BatchOutcome {
    /// Wraps per-source outcomes, deriving the batch error from them.
    #[must_use]
    pub fn from_results(results: Vec<OperationOutcome>) -> Self {
        let error = if results.iter().all(OperationOutcome::ok) {
            String::new()
        } else {
            "one or more operations failed".to_owned()
        };
        Self {
            results,
            collisions: Vec::new(),
            error,
        }
    }

    /// Creates a batch rejected before any source ran.
    #[must_use]
    pub fn rejected(error: impl Into<String>) -> Self {
        Self {
            results: Vec::new(),
            collisions: Vec::new(),
            error: error.into(),
        }
    }

    /// Creates a batch rejected because destinations collided.
    #[must_use]
    pub fn collided(theme_count: usize, collisions: Vec<DestinationCollision>) -> Self {
        Self {
            results: Vec::new(),
            error: format!(
                "cannot materialize {theme_count} theme(s): {} destination(s) collide",
                collisions.len()
            ),
            collisions,
        }
    }

    /// Returns whether every attempted source succeeded.
    #[must_use]
    pub fn ok(&self) -> bool {
        self.error.is_empty()
            && self.collisions.is_empty()
            && self.results.iter().all(OperationOutcome::ok)
    }

    /// Returns the artifacts written by successful results.
    #[must_use]
    pub fn outputs(&self) -> Vec<&str> {
        self.results
            .iter()
            .filter(|result| result.ok() && !result.output.is_empty())
            .map(|result| result.output.as_str())
            .collect()
    }

    /// Returns how many results failed.
    #[must_use]
    pub fn failures(&self) -> usize {
        self.results.iter().filter(|result| !result.ok()).count()
    }
}

/// One planned materialization destination.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterializeDestination {
    /// `res://` theme root to materialize.
    pub source: String,
    /// Confined `res://` artifact to write.
    pub output: String,
}

/// Planning failure for a bulk materialization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MaterializePlanError {
    /// The destination directory is not a confined `res://` directory.
    Directory(String),
    /// Two sources resolve to the same artifact.
    Collisions(Vec<DestinationCollision>),
}

/// Plans one destination per source, rejecting collisions before any write.
///
/// Destinations keep the source file name, so two roots with the same basename
/// would silently overwrite each other. Every destination is checked before
/// anything is written.
pub fn plan_materialize_all(
    sources: &[String],
    directory: &str,
) -> Result<Vec<MaterializeDestination>, MaterializePlanError> {
    let directory =
        profiles::validate_output_directory(directory).map_err(MaterializePlanError::Directory)?;
    let mut planned: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for source in sources {
        let output = format!(
            "{}/{}.tres",
            directory.trim_end_matches('/'),
            file_stem(source)
        );
        planned.entry(output).or_default().push(source.clone());
    }
    let collisions = planned
        .iter()
        .filter(|(_, owners)| owners.len() > 1)
        .map(|(output, sources)| DestinationCollision {
            output: output.clone(),
            sources: sources.clone(),
        })
        .collect::<Vec<_>>();
    if !collisions.is_empty() {
        return Err(MaterializePlanError::Collisions(collisions));
    }
    Ok(planned
        .into_iter()
        .map(|(output, mut owners)| MaterializeDestination {
            source: owners.pop().expect("planned destination has one source"),
            output,
        })
        .collect())
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

#[cfg(test)]
mod tests {
    use super::{
        BatchOutcome, MaterializePlanError, Operation, OperationOutcome, OutcomeStatus,
        plan_materialize_all,
    };

    fn outcome(source: &str, output: &str, ok: bool) -> OperationOutcome {
        OperationOutcome {
            operation: Operation::Materialize,
            profile: String::new(),
            source: source.to_owned(),
            output: output.to_owned(),
            status: if ok {
                OutcomeStatus::Success
            } else {
                OutcomeStatus::Failure
            },
            error: if ok {
                String::new()
            } else {
                "failed".to_owned()
            },
            diagnostics: Vec::new(),
            dependencies: Vec::new(),
            elapsed_ms: 1,
        }
    }

    #[test]
    fn plans_every_source_without_collisions() {
        let sources = vec![
            "res://theme/alpha.tms".to_owned(),
            "res://theme/beta.tms".to_owned(),
        ];
        let planned =
            plan_materialize_all(&sources, "res://generated").expect("distinct file names plan");
        assert_eq!(
            planned
                .iter()
                .map(|destination| destination.output.as_str())
                .collect::<Vec<_>>(),
            ["res://generated/alpha.tres", "res://generated/beta.tres"]
        );
    }

    #[test]
    fn rejects_collisions_and_escaping_directories() {
        let sources = vec![
            "res://theme/alpha.tms".to_owned(),
            "res://theme/nested/alpha.tms".to_owned(),
        ];
        match plan_materialize_all(&sources, "res://generated") {
            Err(MaterializePlanError::Collisions(collisions)) => {
                assert_eq!(collisions.len(), 1);
                assert_eq!(collisions[0].output, "res://generated/alpha.tres");
                assert_eq!(collisions[0].sources.len(), 2);
            }
            other => panic!("collision was not reported: {other:?}"),
        }
        assert!(matches!(
            plan_materialize_all(&sources[..1], "res://.themosis/../escape"),
            Err(MaterializePlanError::Directory(_))
        ));
    }

    #[test]
    fn derives_batch_outputs_and_failures() {
        let batch = BatchOutcome::from_results(vec![
            outcome("res://theme/alpha.tms", "res://generated/alpha.tres", true),
            outcome("res://theme/beta.tms", "", false),
        ]);
        assert!(!batch.ok());
        assert_eq!(batch.outputs(), ["res://generated/alpha.tres"]);
        assert_eq!(batch.failures(), 1);
        assert!(BatchOutcome::from_results(Vec::new()).ok());
        assert!(batch.error.contains("one or more"));
    }
}
