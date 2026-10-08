//! Mapping from compiler failures to wire diagnostics.
//!
//! Every addon entry point reports failures as [`RunnerDiagnostic`] values, so
//! the editor dock, the headless runners, and the CLI runner speak one
//! structured diagnostic model without Godot dictionaries.

use themosis::LoadError;
use themosis_core::Diagnostic;
use themosis_godot::{LineColumn, RunnerDiagnostic, SourceSpan};

use crate::native::backend::ThemeBuildError;

/// Converts loader failures into diagnostics without losing locations.
pub(crate) fn load_diagnostics(error: &LoadError) -> Vec<RunnerDiagnostic> {
    match error {
        LoadError::InvalidPath {
            owner,
            path,
            source,
        } => vec![
            RunnerDiagnostic::new("invalid_source_path", source.to_string()).at_source(
                resource_path_text(owner.as_ref().unwrap_or(path).display().to_string()),
            ),
        ],
        LoadError::Read { path, source } => vec![
            RunnerDiagnostic::new("source_read", source.to_string())
                .at_source(resource_path_text(path.display().to_string())),
        ],
        LoadError::Kdl { path, source } => source
            .errors()
            .iter()
            .flat_map(|error| match error {
                themosis_kdl::ParseError::Syntax(syntax) => {
                    let parser = syntax.parser_error();
                    if parser.diagnostics.is_empty() {
                        return vec![
                            RunnerDiagnostic::new(error.code(), error.to_string())
                                .at_source(resource_path_text(path.display().to_string())),
                        ];
                    }
                    parser
                        .diagnostics
                        .iter()
                        .map(|diagnostic| {
                            let start = diagnostic.span.offset();
                            let end = start.saturating_add(diagnostic.span.len());
                            RunnerDiagnostic::new(
                                error.code(),
                                diagnostic
                                    .message
                                    .as_deref()
                                    .unwrap_or("invalid KDL 2 syntax"),
                            )
                            .at_source(resource_path_text(path.display().to_string()))
                            .with_span(SourceSpan { start, end })
                        })
                        .collect()
                }
                themosis_kdl::ParseError::Structure(structure) => {
                    let diagnostic = RunnerDiagnostic::new(error.code(), structure.to_string())
                        .at_source(resource_path_text(path.display().to_string()));
                    vec![match structure.span() {
                        Some(span) => diagnostic.with_span(SourceSpan {
                            start: span.start(),
                            end: span.end(),
                        }),
                        None => diagnostic,
                    }]
                }
            })
            .collect(),
        LoadError::Tokens { path, source } => source
            .errors()
            .iter()
            .map(|error| {
                let diagnostic = RunnerDiagnostic::new(error.code(), error.to_string())
                    .at_source(resource_path_text(path.display().to_string()));
                match (error.line(), error.column()) {
                    (Some(line), Some(column)) => {
                        diagnostic.at_line_column(LineColumn { line, column })
                    }
                    _ => diagnostic,
                }
            })
            .collect(),
        LoadError::ImportCycle { cycle } => vec![
            RunnerDiagnostic::new("import_cycle", error.to_string()).at_source(
                cycle
                    .first()
                    .map_or_else(String::new, |path| path.display().to_string()),
            ),
        ],
        LoadError::TooManySources => {
            vec![RunnerDiagnostic::new("too_many_sources", error.to_string())]
        }
        LoadError::Compile { source, sources } => source
            .errors()
            .iter()
            .zip(source.metadata())
            .map(|(error, metadata)| {
                let mut message = error.to_string();
                if let Some(suggestion) = metadata.suggestion() {
                    message.push_str("; ");
                    message.push_str(suggestion);
                }
                let mut path = String::new();
                let mut span = None;
                if let Some(label) = metadata.labels().first() {
                    if let Some(source) = sources.get(&label.source()) {
                        path = source.clone();
                    }
                    if let Some(label_span) = label.span() {
                        span = Some(SourceSpan {
                            start: label_span.start(),
                            end: label_span.end(),
                        });
                    }
                }
                let diagnostic = RunnerDiagnostic::new(error.code(), message);
                let diagnostic = match span {
                    Some(span) => diagnostic.with_span(span),
                    None => diagnostic,
                };
                diagnostic.at_source(path)
            })
            .collect(),
    }
}

/// Converts native mapping failures through the same diagnostic model.
pub(crate) fn build_diagnostics(error: &ThemeBuildError) -> Vec<RunnerDiagnostic> {
    match error {
        ThemeBuildError::Preparation(errors) => errors
            .errors()
            .iter()
            .map(|error| RunnerDiagnostic::new(error.code(), error.to_string()))
            .collect(),
        ThemeBuildError::Native(errors) => errors
            .errors()
            .iter()
            .map(|error| {
                RunnerDiagnostic::new(error.code(), error.message()).at_item(
                    error.style(),
                    error.target(),
                    error.state(),
                    error.property(),
                )
            })
            .collect(),
    }
}

fn resource_path_text(path: String) -> String {
    if path.is_empty() || path.starts_with("res://") {
        path
    } else {
        format!("res://{path}")
    }
}
