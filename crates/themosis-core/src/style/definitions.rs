use std::fmt;

use thiserror::Error;

use crate::{Number, SourceId, TokenPath};

/// Non-empty name used by theme, style, state, and property declarations.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Name(String);

impl Name {
    /// Creates a name after rejecting empty or surrounding-whitespace values.
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidName> {
        let value = value.into();
        if value.is_empty() {
            return Err(InvalidName::Empty);
        }
        if value.trim() != value {
            return Err(InvalidName::SurroundingWhitespace);
        }

        Ok(Self(value))
    }

    /// Returns the name as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Name {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Error returned for an invalid declaration name.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum InvalidName {
    /// The name is empty.
    #[error("name is empty")]
    Empty,
    /// The name starts or ends with whitespace.
    #[error("name cannot start or end with whitespace")]
    SurroundingWhitespace,
}

/// Opaque resource reference retained for backend resolution.
///
/// The core does not assign URI schemes or filesystem semantics to this
/// value. A backend is responsible for validating and resolving the reference
/// in its own resource namespace.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceRef(String);

impl ResourceRef {
    /// Creates a non-empty resource reference without surrounding whitespace.
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidResourceRef> {
        let value = value.into();
        if value.is_empty() {
            return Err(InvalidResourceRef::Empty);
        }
        if value.trim() != value {
            return Err(InvalidResourceRef::SurroundingWhitespace);
        }

        Ok(Self(value))
    }

    /// Returns the backend-defined reference text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ResourceRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Error returned for an unusable backend resource reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum InvalidResourceRef {
    /// The reference is empty.
    #[error("resource reference is empty")]
    Empty,
    /// The reference starts or ends with whitespace.
    #[error("resource reference cannot start or end with whitespace")]
    SurroundingWhitespace,
}

/// Unresolved value assigned to a component property.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StyleValue {
    /// Boolean literal.
    Boolean(bool),
    /// Unitless numeric literal.
    Number(Number),
    /// UTF-8 string literal.
    String(String),
    /// Reference to a design token.
    Token(TokenPath),
    /// Opaque reference resolved by the selected backend.
    Resource(ResourceRef),
}

/// One named property assignment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PropertyAssignment {
    name: Name,
    value: StyleValue,
}

impl PropertyAssignment {
    /// Creates a property assignment.
    #[must_use]
    pub const fn new(name: Name, value: StyleValue) -> Self {
        Self { name, value }
    }

    /// Returns the property name.
    #[must_use]
    pub const fn name(&self) -> &Name {
        &self.name
    }

    /// Returns the unresolved property value.
    #[must_use]
    pub const fn value(&self) -> &StyleValue {
        &self.value
    }
}

/// Explicit control state and its property overrides.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleState {
    name: Name,
    properties: Vec<PropertyAssignment>,
}

impl StyleState {
    /// Creates an explicit state.
    #[must_use]
    pub const fn new(name: Name, properties: Vec<PropertyAssignment>) -> Self {
        Self { name, properties }
    }

    /// Returns the state name.
    #[must_use]
    pub const fn name(&self) -> &Name {
        &self.name
    }

    /// Returns property overrides in source order.
    #[must_use]
    pub fn properties(&self) -> &[PropertyAssignment] {
        &self.properties
    }
}

/// Component style and its explicit states.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleDefinition {
    name: Name,
    target: Name,
    extends: Option<Name>,
    properties: Vec<PropertyAssignment>,
    states: Vec<StyleState>,
}

impl StyleDefinition {
    /// Creates a component style.
    #[must_use]
    pub const fn new(
        name: Name,
        target: Name,
        extends: Option<Name>,
        properties: Vec<PropertyAssignment>,
        states: Vec<StyleState>,
    ) -> Self {
        Self {
            name,
            target,
            extends,
            properties,
            states,
        }
    }

    /// Returns the style or type-variation name.
    #[must_use]
    pub const fn name(&self) -> &Name {
        &self.name
    }

    /// Returns the backend target type selected by the style.
    #[must_use]
    pub const fn target(&self) -> &Name {
        &self.target
    }

    /// Returns the optional parent style name.
    #[must_use]
    pub const fn extends(&self) -> Option<&Name> {
        self.extends.as_ref()
    }

    /// Returns base property assignments in source order.
    #[must_use]
    pub fn properties(&self) -> &[PropertyAssignment] {
        &self.properties
    }

    /// Returns explicit states in source order.
    #[must_use]
    pub fn states(&self) -> &[StyleState] {
        &self.states
    }
}

/// Component styles and source declarations decoded from one KDL document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleDocument {
    source: SourceId,
    name: Name,
    token_sources: Vec<String>,
    imports: Vec<String>,
    styles: Vec<StyleDefinition>,
}

impl StyleDocument {
    /// Creates a source document while preserving declaration order.
    #[must_use]
    pub const fn new(
        source: SourceId,
        name: Name,
        token_sources: Vec<String>,
        imports: Vec<String>,
        styles: Vec<StyleDefinition>,
    ) -> Self {
        Self {
            source,
            name,
            token_sources,
            imports,
            styles,
        }
    }

    /// Returns the source identity.
    #[must_use]
    pub const fn source(&self) -> SourceId {
        self.source
    }

    /// Returns the theme name.
    #[must_use]
    pub const fn name(&self) -> &Name {
        &self.name
    }

    /// Returns declared token JSON paths in source order.
    #[must_use]
    pub fn token_sources(&self) -> &[String] {
        &self.token_sources
    }

    /// Returns imported KDL paths in source order.
    #[must_use]
    pub fn imports(&self) -> &[String] {
        &self.imports
    }

    /// Returns component styles in source order.
    #[must_use]
    pub fn styles(&self) -> &[StyleDefinition] {
        &self.styles
    }
}

#[cfg(test)]
mod tests {
    use super::{InvalidName, InvalidResourceRef, Name, ResourceRef};

    #[test]
    fn rejects_empty_and_padded_names() {
        assert_eq!(
            Name::new("").expect_err("name is empty"),
            InvalidName::Empty
        );
        assert_eq!(
            Name::new(" Button ").expect_err("name is padded"),
            InvalidName::SurroundingWhitespace
        );
    }

    #[test]
    fn validates_backend_neutral_resource_references() {
        let reference =
            ResourceRef::new("theme://fonts/ui").expect("reference is non-empty and trimmed");

        assert_eq!(reference.as_str(), "theme://fonts/ui");
        assert_eq!(
            ResourceRef::new("").expect_err("empty reference is invalid"),
            InvalidResourceRef::Empty
        );
        assert_eq!(
            ResourceRef::new(" padded ").expect_err("padded reference is invalid"),
            InvalidResourceRef::SurroundingWhitespace
        );
    }
}
