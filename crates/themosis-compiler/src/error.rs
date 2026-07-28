use themosis_core::{CompiledValueKind, Name, TokenKind, TokenPath};
use thiserror::Error;

/// One semantic compilation error.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum CompileError {
    /// Style compilation was requested without any style documents.
    #[error("no style documents supplied")]
    NoStyleDocuments,
    /// Style documents in one compilation use different theme names.
    #[error("style document uses theme '{found}', expected '{expected}'")]
    ThemeNameMismatch {
        /// Theme name established by the first document.
        expected: Name,
        /// Conflicting theme name.
        found: Name,
    },
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
    /// A component style name was declared more than once.
    #[error("duplicate style '{style}'")]
    DuplicateStyle {
        /// Duplicated style name.
        style: Name,
    },
    /// A style extends a style that does not exist.
    #[error("style '{style}' extends missing style '{parent}'")]
    MissingParent {
        /// Child style.
        style: Name,
        /// Missing parent style.
        parent: Name,
    },
    /// A style inheritance graph contains a cycle.
    #[error("style inheritance cycle: {}", format_cycle(.cycle))]
    InheritanceCycle {
        /// Closed path through the cycle, with the first name repeated last.
        cycle: Vec<Name>,
    },
    /// A child and its parent target different Godot control types.
    #[error("style '{style}' targets '{target}' but parent '{parent}' targets '{parent_target}'")]
    TargetMismatch {
        /// Child style.
        style: Name,
        /// Child control target.
        target: Name,
        /// Parent style.
        parent: Name,
        /// Parent control target.
        parent_target: Name,
    },
    /// A state name was declared more than once in one style.
    #[error("style '{style}' has duplicate state '{state}'")]
    DuplicateState {
        /// Containing style.
        style: Name,
        /// Duplicated state name.
        state: Name,
    },
    /// A property was assigned more than once in one declaration block.
    #[error("{}", format_duplicate_property(.style, .state, .property))]
    DuplicateProperty {
        /// Containing style.
        style: Name,
        /// State containing the property, or `None` for base properties.
        state: Option<Name>,
        /// Duplicated property name.
        property: Name,
    },
    /// A style property references an unresolved token.
    #[error("{}", format_missing_token(.style, .state, .property, .token))]
    MissingToken {
        /// Containing style.
        style: Name,
        /// State containing the property, or `None` for base properties.
        state: Option<Name>,
        /// Property containing the reference.
        property: Name,
        /// Missing token path.
        token: TokenPath,
    },
    /// A property override changes the established value kind.
    #[error(
        "{}",
        format_property_type_mismatch(.style, .state, .property, .expected, .actual)
    )]
    PropertyTypeMismatch {
        /// Containing style.
        style: Name,
        /// State containing the property, or `None` for base properties.
        state: Option<Name>,
        /// Incompatible property.
        property: Name,
        /// Kind established by a base or inherited value.
        expected: CompiledValueKind,
        /// Kind supplied by the override.
        actual: CompiledValueKind,
    },
}

impl CompileError {
    /// Returns the stable diagnostic code for this semantic failure.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::NoStyleDocuments => "TMS2001",
            Self::ThemeNameMismatch { .. } => "TMS2002",
            Self::DuplicateToken { .. } => "TMS2101",
            Self::MissingAlias { .. } => "TMS2102",
            Self::AliasCycle { .. } => "TMS2103",
            Self::TypeMismatch { .. } => "TMS2104",
            Self::DuplicateStyle { .. } => "TMS2201",
            Self::MissingParent { .. } => "TMS2202",
            Self::InheritanceCycle { .. } => "TMS2203",
            Self::TargetMismatch { .. } => "TMS2204",
            Self::DuplicateState { .. } => "TMS2205",
            Self::DuplicateProperty { .. } => "TMS2206",
            Self::MissingToken { .. } => "TMS2207",
            Self::PropertyTypeMismatch { .. } => "TMS2208",
        }
    }
}

fn format_cycle<T: std::fmt::Display>(cycle: &[T]) -> String {
    cycle
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" -> ")
}

fn format_duplicate_property(style: &Name, state: &Option<Name>, property: &Name) -> String {
    match state {
        Some(state) => {
            format!("style '{style}' state '{state}' has duplicate property '{property}'")
        }
        None => format!("style '{style}' has duplicate property '{property}'"),
    }
}

fn format_missing_token(
    style: &Name,
    state: &Option<Name>,
    property: &Name,
    token: &TokenPath,
) -> String {
    match state {
        Some(state) => format!(
            "style '{style}' state '{state}' property '{property}' references missing token '{token}'"
        ),
        None => format!("style '{style}' property '{property}' references missing token '{token}'"),
    }
}

fn format_property_type_mismatch(
    style: &Name,
    state: &Option<Name>,
    property: &Name,
    expected: &CompiledValueKind,
    actual: &CompiledValueKind,
) -> String {
    match state {
        Some(state) => format!(
            "style '{style}' state '{state}' property '{property}' changes kind from {expected:?} to {actual:?}"
        ),
        None => format!(
            "style '{style}' property '{property}' changes kind from {expected:?} to {actual:?}"
        ),
    }
}
