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

/// All errors found while decoding one token document.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
#[error("{}", format_parse_errors(.0))]
pub struct ParseErrors(Vec<ParseError>);

impl ParseErrors {
    pub(crate) fn one(error: ParseError) -> Self {
        Self(vec![error])
    }

    pub(crate) fn many(errors: Vec<ParseError>) -> Self {
        Self(errors)
    }

    /// Returns errors in deterministic document traversal order.
    #[must_use]
    pub fn errors(&self) -> &[ParseError] {
        &self.0
    }
}

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

fn format_parse_errors(errors: &[ParseError]) -> String {
    errors
        .iter()
        .map(|error| format!("{error}"))
        .collect::<Vec<_>>()
        .join("\n")
}
