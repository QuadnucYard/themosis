use themosis_core::{Diagnostic, Errors};
use thiserror::Error;

/// One syntax or structural token-document error.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
#[error("{}", format_parse_error(.path, .line, .column, .message))]
pub struct ParseError {
    path: String,
    line: Option<usize>,
    column: Option<usize>,
    message: String,
}

impl ParseError {
    pub(crate) fn at(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            line: None,
            column: None,
            message: message.into(),
        }
    }

    pub(crate) fn syntax(line: usize, column: usize, message: String) -> Self {
        Self {
            path: "$".to_owned(),
            line: Some(line),
            column: Some(column),
            message,
        }
    }

    /// Returns the JSON path associated with the error.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the one-based line for a JSON syntax error.
    #[must_use]
    pub const fn line(&self) -> Option<usize> {
        self.line
    }

    /// Returns the one-based column for a JSON syntax error.
    #[must_use]
    pub const fn column(&self) -> Option<usize> {
        self.column
    }

    /// Returns the error message without location decoration.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Diagnostic for ParseError {
    fn code(&self) -> &str {
        if self.line.is_some() {
            "TMS1101"
        } else {
            "TMS1102"
        }
    }
}

/// All errors found while decoding one token document.
pub type ParseErrors = Errors<ParseError>;

fn format_parse_error(
    path: &str,
    line: &Option<usize>,
    column: &Option<usize>,
    message: &str,
) -> String {
    if let (Some(line), Some(column)) = (line.as_ref(), column.as_ref()) {
        format!("{message} at line {line}, column {column}")
    } else {
        format!("{path}: {message}")
    }
}
