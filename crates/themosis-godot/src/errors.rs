use themosis_core::{Diagnostic, Errors, Name, ResourceRef};
use thiserror::Error;

/// One portable Godot build-plan failure.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum BackendError {
    /// A compiler value category has no Godot theme-item representation.
    #[error("{}", format_unsupported_value(.style, .target, .state.as_ref(), .property, .value))]
    UnsupportedValue {
        /// Style being mapped.
        style: Name,
        /// Native control target.
        target: Name,
        /// State containing the property, or `None` for base properties.
        state: Option<Name>,
        /// Unsupported native item name.
        property: Name,
        /// Compiled value kind.
        value: &'static str,
    },
    /// A state tried to change the same native item used by the base style.
    #[error(
        "style '{style}' state '{state}' changes base theme item '{property}'; use the exact state-specific Godot item name instead"
    )]
    StateOverridesBaseItem {
        /// Style being mapped.
        style: Name,
        /// State containing the override.
        state: Name,
        /// Reused base item name.
        property: Name,
    },
    /// A numeric theme item is not a whole number of pixels.
    #[error("style '{style}' property '{property}' must be {expected}")]
    InvalidInteger {
        /// Style being mapped.
        style: Name,
        /// Invalid native item.
        property: Name,
        /// Required numeric constraint.
        expected: &'static str,
    },
    /// A resource reference is not in Godot's project resource namespace.
    #[error(
        "resource reference '{reference}' must use a non-empty 'res://' or 'uid://' Godot path"
    )]
    InvalidResourceReference {
        /// Backend-neutral reference supplied by the compiler.
        reference: ResourceRef,
    },
}

impl Diagnostic for BackendError {
    fn code(&self) -> &str {
        match self {
            Self::UnsupportedValue { .. } => "TMS3001",
            Self::StateOverridesBaseItem { .. } => "TMS3002",
            Self::InvalidInteger { .. } => "TMS3003",
            Self::InvalidResourceReference { .. } => "TMS3004",
        }
    }
}

fn format_unsupported_value(
    style: &Name,
    target: &Name,
    state: Option<&Name>,
    property: &Name,
    value: &'static str,
) -> String {
    match state {
        Some(state) => format!(
            "style '{style}' state '{state}' property '{property}' cannot map {value} values to a Godot theme item on target '{target}'"
        ),
        None => format!(
            "style '{style}' property '{property}' cannot map {value} values to a Godot theme item on target '{target}'"
        ),
    }
}

/// Portable planning failures collected in deterministic style/property order.
pub type BackendErrors = Errors<BackendError>;
