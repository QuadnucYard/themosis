use thiserror::Error;

use crate::{Color, Dimension, Number, SourceId, TokenPath};

/// Token types supported by the initial JSON contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenKind {
    /// Boolean value.
    Boolean,
    /// sRGB color value.
    Color,
    /// Pixel or rem dimension.
    Dimension,
    /// Unitless finite number.
    Number,
    /// UTF-8 string.
    String,
}

/// A typed literal token value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TokenValue {
    /// Boolean literal.
    Boolean(bool),
    /// sRGB color literal.
    Color(Color),
    /// Dimension literal.
    Dimension(Dimension),
    /// Unitless number literal.
    Number(Number),
    /// String literal.
    String(String),
}

impl TokenValue {
    /// Returns the type of this literal.
    #[must_use]
    pub const fn kind(&self) -> TokenKind {
        match self {
            Self::Boolean(_) => TokenKind::Boolean,
            Self::Color(_) => TokenKind::Color,
            Self::Dimension(_) => TokenKind::Dimension,
            Self::Number(_) => TokenKind::Number,
            Self::String(_) => TokenKind::String,
        }
    }
}

/// Unresolved token value from a source document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TokenExpression {
    /// A typed literal.
    Literal(TokenValue),
    /// Reference to another token path.
    Alias(TokenPath),
}

/// One typed token declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenDefinition {
    path: TokenPath,
    kind: TokenKind,
    expression: TokenExpression,
}

impl TokenDefinition {
    /// Creates a declaration while enforcing literal type consistency.
    pub fn new(
        path: TokenPath,
        kind: TokenKind,
        expression: TokenExpression,
    ) -> Result<Self, InvalidTokenDefinition> {
        if let TokenExpression::Literal(value) = &expression {
            if value.kind() != kind {
                return Err(InvalidTokenDefinition {
                    declared: kind,
                    actual: value.kind(),
                });
            }
        }

        Ok(Self {
            path,
            kind,
            expression,
        })
    }

    /// Returns the token path.
    #[must_use]
    pub const fn path(&self) -> &TokenPath {
        &self.path
    }

    /// Returns the declared token type.
    #[must_use]
    pub const fn kind(&self) -> TokenKind {
        self.kind
    }

    /// Returns the unresolved token expression.
    #[must_use]
    pub const fn expression(&self) -> &TokenExpression {
        &self.expression
    }
}

/// Error returned when a literal does not match its declared token type.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
#[error("declared token type {declared:?} does not match literal type {actual:?}")]
pub struct InvalidTokenDefinition {
    declared: TokenKind,
    actual: TokenKind,
}

/// Tokens decoded from one JSON source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenDocument {
    source: SourceId,
    tokens: Vec<TokenDefinition>,
}

impl TokenDocument {
    /// Creates a document whose tokens are already in canonical order.
    #[must_use]
    pub fn new(source: SourceId, mut tokens: Vec<TokenDefinition>) -> Self {
        tokens.sort_by(|left, right| left.path.cmp(&right.path));
        Self { source, tokens }
    }

    /// Returns the source identity.
    #[must_use]
    pub const fn source(&self) -> SourceId {
        self.source
    }

    /// Returns declarations in canonical token-path order.
    #[must_use]
    pub fn tokens(&self) -> &[TokenDefinition] {
        &self.tokens
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn rejects_mismatched_literal_types() {
        let error = TokenDefinition::new(
            TokenPath::from_str("enabled").expect("path is valid"),
            TokenKind::Number,
            TokenExpression::Literal(TokenValue::Boolean(true)),
        )
        .expect_err("literal type does not match");

        assert_eq!(
            error.to_string(),
            "declared token type Number does not match literal type Boolean"
        );
    }
}
