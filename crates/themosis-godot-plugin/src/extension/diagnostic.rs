use std::collections::BTreeSet;

use godot::{classes::Theme, prelude::*};
use themosis::LoadError;
use themosis_core::Diagnostic;

use crate::backend::ThemeBuildError;

#[derive(Clone, Debug)]
pub(super) struct EditorDiagnostic {
    code: String,
    message: String,
    path: String,
    span_start: Option<usize>,
    span_end: Option<usize>,
    line: Option<usize>,
    column: Option<usize>,
}

impl EditorDiagnostic {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            path: String::new(),
            span_start: None,
            span_end: None,
            line: None,
            column: None,
        }
    }

    pub fn at_path(mut self, path: impl Into<String>) -> Self {
        self.path = resource_path_text(path.into());
        self
    }

    pub fn with_span(mut self, start: usize, end: usize) -> Self {
        self.span_start = Some(start);
        self.span_end = Some(end);
        self
    }

    pub const fn with_location(mut self, line: Option<usize>, column: Option<usize>) -> Self {
        self.line = line;
        self.column = column;
        self
    }

    pub fn to_dictionary(&self) -> VarDictionary {
        let mut value = VarDictionary::new();
        value.set("severity", "error");
        value.set("code", self.code.as_str());
        value.set("message", self.message.as_str());
        value.set("path", self.path.as_str());
        value.set(
            "span_start",
            self.span_start.map_or(-1_i64, |value| value as i64),
        );
        value.set(
            "span_end",
            self.span_end.map_or(-1_i64, |value| value as i64),
        );
        value.set("line", self.line.map_or(-1_i64, |value| value as i64));
        value.set("column", self.column.map_or(-1_i64, |value| value as i64));
        value
    }
}

#[derive(Debug)]
pub(super) struct GenerationAttempt {
    pub(super) dependencies: BTreeSet<String>,
    pub(super) result: Result<Gd<Theme>, GenerationFailure>,
}

#[derive(Debug)]
pub(super) struct GenerationFailure {
    pub(super) message: String,
    pub(super) diagnostics: Vec<EditorDiagnostic>,
}

pub(super) fn load_diagnostics(error: &LoadError) -> Vec<EditorDiagnostic> {
    match error {
        LoadError::InvalidPath {
            owner,
            path,
            source,
        } => vec![
            EditorDiagnostic::new("invalid_source_path", source.to_string())
                .at_path(owner.as_ref().unwrap_or(path).display().to_string()),
        ],
        LoadError::Read { path, source } => vec![
            EditorDiagnostic::new("source_read", source.to_string())
                .at_path(path.display().to_string()),
        ],
        LoadError::Kdl { path, source } => source
            .errors()
            .iter()
            .flat_map(|error| match error {
                themosis_kdl::ParseError::Syntax(syntax) => {
                    let parser = syntax.parser_error();
                    if parser.diagnostics.is_empty() {
                        return vec![
                            EditorDiagnostic::new(error.code(), error.to_string())
                                .at_path(path.display().to_string()),
                        ];
                    }
                    parser
                        .diagnostics
                        .iter()
                        .map(|diagnostic| {
                            let start = diagnostic.span.offset();
                            let end = start.saturating_add(diagnostic.span.len());
                            EditorDiagnostic::new(
                                error.code(),
                                diagnostic
                                    .message
                                    .as_deref()
                                    .unwrap_or("invalid KDL 2 syntax"),
                            )
                            .at_path(path.display().to_string())
                            .with_span(start, end)
                        })
                        .collect()
                }
                themosis_kdl::ParseError::Structure(structure) => {
                    let diagnostic = EditorDiagnostic::new(error.code(), structure.to_string())
                        .at_path(path.display().to_string());
                    vec![structure.span().map_or(diagnostic.clone(), |span| {
                        diagnostic.with_span(span.start(), span.end())
                    })]
                }
            })
            .collect(),
        LoadError::Tokens { path, source } => source
            .errors()
            .iter()
            .map(|error| {
                EditorDiagnostic::new(error.code(), error.to_string())
                    .at_path(path.display().to_string())
                    .with_location(error.line(), error.column())
            })
            .collect(),
        LoadError::ImportCycle { cycle } => vec![
            EditorDiagnostic::new("import_cycle", error.to_string()).at_path(
                cycle
                    .first()
                    .map_or_else(String::new, |path| path.display().to_string()),
            ),
        ],
        LoadError::TooManySources => {
            vec![EditorDiagnostic::new("too_many_sources", error.to_string())]
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
                let mut diagnostic = EditorDiagnostic::new(error.code(), message);
                if let Some(label) = metadata.labels().first() {
                    if let Some(path) = sources.get(&label.source()) {
                        diagnostic = diagnostic.at_path(path.clone());
                    }
                    if let Some(span) = label.span() {
                        diagnostic = diagnostic.with_span(span.start(), span.end());
                    }
                }
                diagnostic
            })
            .collect(),
    }
}

pub(super) fn build_diagnostics(error: &ThemeBuildError) -> Vec<EditorDiagnostic> {
    match error {
        ThemeBuildError::Preparation(errors) => errors
            .errors()
            .iter()
            .map(|error| EditorDiagnostic::new(error.code(), error.to_string()))
            .collect(),
        ThemeBuildError::Native(errors) => errors
            .errors()
            .iter()
            .map(|error| EditorDiagnostic::new(error.code(), error.to_string()))
            .collect(),
        ThemeBuildError::Builder(message) => {
            vec![EditorDiagnostic::new("native_builder", message)]
        }
    }
}

fn resource_path_text(path: String) -> String {
    if path.is_empty() || path.starts_with("res://") {
        path
    } else {
        format!("res://{path}")
    }
}
