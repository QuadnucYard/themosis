//! Core domain types for the Themosis design-system compiler.
//!
//! This crate owns small, format-independent primitives shared by the JSON
//! token and KDL style inputs. It performs no parsing or filesystem access.

#![forbid(unsafe_code)]

mod source;
mod token;

pub use source::{InvalidSpan, SourceId, Span};
pub use token::{
    Color, Dimension, DimensionUnit, InvalidColor, InvalidNumber, InvalidTokenDefinition,
    InvalidTokenPath, Number, TokenDefinition, TokenDocument, TokenExpression, TokenKind,
    TokenPath, TokenValue,
};
