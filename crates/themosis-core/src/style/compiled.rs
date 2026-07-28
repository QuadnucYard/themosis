use std::collections::BTreeMap;

use super::definitions::{Name, ResourceRef};
use crate::{Color, Dimension, Number, ResolvedTokens, TokenValue};

/// Value available to a backend after semantic compilation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompiledValue {
    /// Boolean value.
    Boolean(bool),
    /// sRGB color value.
    Color(Color),
    /// Pixel or rem dimension.
    Dimension(Dimension),
    /// Unitless number.
    Number(Number),
    /// UTF-8 string.
    String(String),
    /// Opaque backend resource reference.
    Resource(ResourceRef),
}

impl CompiledValue {
    /// Returns the value kind used to validate property overrides.
    #[must_use]
    pub const fn kind(&self) -> CompiledValueKind {
        match self {
            Self::Boolean(_) => CompiledValueKind::Boolean,
            Self::Color(_) => CompiledValueKind::Color,
            Self::Dimension(_) => CompiledValueKind::Dimension,
            Self::Number(_) => CompiledValueKind::Number,
            Self::String(_) => CompiledValueKind::String,
            Self::Resource(_) => CompiledValueKind::Resource,
        }
    }
}

impl From<TokenValue> for CompiledValue {
    fn from(value: TokenValue) -> Self {
        match value {
            TokenValue::Boolean(value) => Self::Boolean(value),
            TokenValue::Color(value) => Self::Color(value),
            TokenValue::Dimension(value) => Self::Dimension(value),
            TokenValue::Number(value) => Self::Number(value),
            TokenValue::String(value) => Self::String(value),
        }
    }
}

/// Discriminant for a compiled property value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompiledValueKind {
    /// Boolean value.
    Boolean,
    /// sRGB color value.
    Color,
    /// Pixel or rem dimension.
    Dimension,
    /// Unitless number.
    Number,
    /// UTF-8 string.
    String,
    /// Opaque backend resource reference.
    Resource,
}

/// Explicit state with its fully expanded property set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledState {
    name: Name,
    properties: BTreeMap<Name, CompiledValue>,
}

impl CompiledState {
    /// Creates a compiled state.
    #[must_use]
    pub const fn new(name: Name, properties: BTreeMap<Name, CompiledValue>) -> Self {
        Self { name, properties }
    }

    /// Returns the state name.
    #[must_use]
    pub const fn name(&self) -> &Name {
        &self.name
    }

    /// Returns fully expanded properties in name order.
    #[must_use]
    pub const fn properties(&self) -> &BTreeMap<Name, CompiledValue> {
        &self.properties
    }
}

/// Canonical component style ready for backend mapping.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledStyle {
    name: Name,
    target: Name,
    properties: BTreeMap<Name, CompiledValue>,
    states: BTreeMap<Name, CompiledState>,
}

impl CompiledStyle {
    /// Creates a compiled component style.
    #[must_use]
    pub const fn new(
        name: Name,
        target: Name,
        properties: BTreeMap<Name, CompiledValue>,
        states: BTreeMap<Name, CompiledState>,
    ) -> Self {
        Self {
            name,
            target,
            properties,
            states,
        }
    }

    /// Returns the style or type-variation name.
    #[must_use]
    pub const fn name(&self) -> &Name {
        &self.name
    }

    /// Returns the backend target type.
    #[must_use]
    pub const fn target(&self) -> &Name {
        &self.target
    }

    /// Returns base properties in name order.
    #[must_use]
    pub const fn properties(&self) -> &BTreeMap<Name, CompiledValue> {
        &self.properties
    }

    /// Returns explicit states in name order.
    #[must_use]
    pub const fn states(&self) -> &BTreeMap<Name, CompiledState> {
        &self.states
    }
}

/// Canonical theme produced by semantic compilation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledTheme {
    name: Name,
    tokens: ResolvedTokens,
    styles: BTreeMap<Name, CompiledStyle>,
}

impl CompiledTheme {
    /// Creates a compiled theme.
    #[must_use]
    pub const fn new(
        name: Name,
        tokens: ResolvedTokens,
        styles: BTreeMap<Name, CompiledStyle>,
    ) -> Self {
        Self {
            name,
            tokens,
            styles,
        }
    }

    /// Returns the theme name.
    #[must_use]
    pub const fn name(&self) -> &Name {
        &self.name
    }

    /// Returns the resolved design tokens.
    #[must_use]
    pub const fn tokens(&self) -> &ResolvedTokens {
        &self.tokens
    }

    /// Returns component styles in name order.
    #[must_use]
    pub const fn styles(&self) -> &BTreeMap<Name, CompiledStyle> {
        &self.styles
    }
}
