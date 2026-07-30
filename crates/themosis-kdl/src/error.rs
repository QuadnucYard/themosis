use themosis_core::{Diagnostic, Errors, Span};
use thiserror::Error;

/// KDL syntax or structure-conversion failure.
#[derive(Debug, Error)]
pub enum ParseError {
    /// KDL syntax error reported by the KDL parser.
    #[error(transparent)]
    Syntax(#[from] SyntaxError),
    /// One parsed KDL declaration that violates the format contract or core invariants.
    #[error(transparent)]
    Structure(#[from] StructureError),
}

impl Diagnostic for ParseError {
    fn code(&self) -> &str {
        match self {
            Self::Syntax(_) => "TMS1001",
            Self::Structure(_) => "TMS1002",
        }
    }
}

/// All errors found while parsing one KDL document.
pub type ParseErrors = Errors<ParseError>;

/// One filename-aware KDL 2 syntax failure.
#[derive(Debug, Error)]
#[error("{}", format_syntax_error(.file_name, .source))]
pub struct SyntaxError {
    file_name: String,
    #[source]
    source: Box<kdl::KdlError>,
}

impl SyntaxError {
    pub(crate) fn new(file_name: impl Into<String>, source: kdl::KdlError) -> Self {
        Self {
            file_name: file_name.into(),
            source: Box::new(source),
        }
    }

    /// Returns the source name supplied to the parser.
    #[must_use]
    pub fn file_name(&self) -> &str {
        &self.file_name
    }

    /// Returns the underlying KDL parser error.
    #[must_use]
    pub fn parser_error(&self) -> &kdl::KdlError {
        &self.source
    }
}

impl Diagnostic for SyntaxError {
    fn code(&self) -> &str {
        "TMS1001"
    }
}

/// One decoded value that violates the KDL format contract.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
#[error("{context}: {message}{}", format_span(.span))]
pub struct StructureError {
    context: String,
    message: String,
    span: Option<Span>,
}

impl StructureError {
    pub(crate) fn new(context: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            context: context.into(),
            message: message.into(),
            span: None,
        }
    }

    pub(crate) fn at(context: impl Into<String>, message: impl Into<String>, span: Span) -> Self {
        Self {
            context: context.into(),
            message: message.into(),
            span: Some(span),
        }
    }

    /// Returns the declaration context.
    #[must_use]
    pub fn context(&self) -> &str {
        &self.context
    }

    /// Returns the error message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the invalid declaration span when available.
    #[must_use]
    pub const fn span(&self) -> Option<Span> {
        self.span
    }
}

impl Diagnostic for StructureError {
    fn code(&self) -> &str {
        "TMS1002"
    }
}

fn format_span(span: &Option<Span>) -> String {
    span.as_ref().map_or_else(String::new, |span| {
        format!(" at bytes {}..{}", span.start(), span.end())
    })
}

fn format_syntax_error(file_name: &str, error: &kdl::KdlError) -> String {
    let diagnostics = error
        .diagnostics
        .iter()
        .map(|diagnostic| {
            let start = diagnostic.span.offset();
            let end = start
                .checked_add(diagnostic.span.len())
                .expect("KDL diagnostic spans fit within their input");
            let message = diagnostic
                .message
                .as_deref()
                .unwrap_or("invalid KDL 2 syntax");
            format!("{file_name}: {message} at bytes {start}..{end}")
        })
        .collect::<Vec<_>>();

    if diagnostics.is_empty() {
        format!("{file_name}: invalid KDL 2 syntax")
    } else {
        diagnostics.join("\n")
    }
}
