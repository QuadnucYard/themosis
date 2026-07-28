use themosis_core::Span;
use thiserror::Error;

/// KDL decoding or structure-conversion failure.
#[derive(Debug, Error)]
pub enum ParseError {
    /// KDL syntax or derive-schema error reported by knus.
    #[error("{0}")]
    Decode(#[source] Box<knus::Error>),
    /// Values that decoded from KDL but violate core invariants.
    #[error("{0}")]
    Structure(#[source] StructureErrors),
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

/// All structure errors collected from one KDL document.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
#[error("{}", format_structure_errors(.0))]
pub struct StructureErrors(pub(crate) Vec<StructureError>);

impl StructureErrors {
    /// Returns errors in source traversal order.
    #[must_use]
    pub fn errors(&self) -> &[StructureError] {
        &self.0
    }
}

fn format_span(span: &Option<Span>) -> String {
    span.as_ref().map_or_else(String::new, |span| {
        format!(" at bytes {}..{}", span.start(), span.end())
    })
}

fn format_structure_errors(errors: &[StructureError]) -> String {
    errors
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}
