use std::fmt;

use themosis_core::{TokenKind, TokenPath};
use thiserror::Error;

/// One token semantic error.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum CompileError {
    /// A token path was declared more than once.
    #[error("duplicate token '{path}'")]
    DuplicateToken {
        /// Duplicated path.
        path: TokenPath,
    },
    /// An alias target does not exist.
    #[error("token '{token}' references missing token '{target}'")]
    MissingAlias {
        /// Token containing the alias.
        token: TokenPath,
        /// Missing target path.
        target: TokenPath,
    },
    /// A token alias graph contains a cycle.
    #[error("token alias cycle: {}", format_cycle(.cycle))]
    AliasCycle {
        /// Closed path through the cycle, with the first path repeated last.
        cycle: Vec<TokenPath>,
    },
    /// A resolved alias value has the wrong declared type.
    #[error("token '{token}' declares {declared:?} but resolves to {actual:?}")]
    TypeMismatch {
        /// Token containing the incompatible alias.
        token: TokenPath,
        /// Type declared by the aliasing token.
        declared: TokenKind,
        /// Type produced by the target.
        actual: TokenKind,
    },
}

fn format_cycle<T: std::fmt::Display>(cycle: &[T]) -> String {
    cycle
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" -> ")
}

/// All token semantic errors collected in deterministic order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileErrors(pub(crate) Vec<CompileError>);

impl CompileErrors {
    /// Returns collected errors.
    #[must_use]
    pub fn errors(&self) -> &[CompileError] {
        &self.0
    }
}

impl fmt::Display for CompileErrors {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, error) in self.0.iter().enumerate() {
            if index > 0 {
                formatter.write_str("\n")?;
            }
            write!(formatter, "{error}")?;
        }
        Ok(())
    }
}

impl std::error::Error for CompileErrors {}
