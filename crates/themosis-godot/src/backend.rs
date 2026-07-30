use std::collections::BTreeMap;

use serde::{Serialize, Serializer, ser::SerializeStruct};
use themosis_core::{
    Color, CompiledStyle, CompiledTheme, CompiledValue, DimensionUnit, Name, ResourceRef,
};

use crate::{BackendError, BackendErrors};

/// Portable, serializable instructions for building a Godot theme.
///
/// Item categories remain candidates until a running Godot engine resolves
/// them against its own classes and default theme.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GodotBuildPlan {
    /// Compiled theme name written to the plan's `theme` field.
    name: Name,
    /// Planned styles keyed by name to preserve deterministic ordering.
    styles: BTreeMap<Name, PlannedStyle>,
}

impl GodotBuildPlan {
    /// Build-plan interchange format version.
    pub const SCHEMA_VERSION: u64 = 2;

    /// Returns the compiled theme name.
    #[must_use]
    pub const fn name(&self) -> &Name {
        &self.name
    }

    /// Returns planned styles in deterministic name order.
    #[must_use]
    pub const fn styles(&self) -> &BTreeMap<Name, PlannedStyle> {
        &self.styles
    }
}

// This is manual because the wire format includes an associated schema version,
// renames `name` to `theme`, and encodes the styles map as an ordered array.
impl Serialize for GodotBuildPlan {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut plan = serializer.serialize_struct("GodotBuildPlan", 3)?;
        plan.serialize_field("schema_version", &Self::SCHEMA_VERSION)?;
        plan.serialize_field("theme", self.name.as_str())?;
        plan.serialize_field("styles", &self.styles.values().collect::<Vec<_>>())?;
        plan.end()
    }
}

/// One default control type or named variation in a portable Godot build plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedStyle {
    /// Theme type name. A name equal to `target` styles that native type by default.
    name: Name,
    /// Native `Control` class styled directly or extended by the variation.
    target: Name,
    /// Normalized theme items emitted in source order.
    items: Vec<PlannedItem>,
}

impl PlannedStyle {
    /// Returns the theme type or variation name.
    #[must_use]
    pub const fn name(&self) -> &Name {
        &self.name
    }

    /// Returns the requested native `Control` target.
    #[must_use]
    pub const fn target(&self) -> &Name {
        &self.target
    }

    /// Returns planned items in deterministic source order.
    #[must_use]
    pub fn items(&self) -> &[PlannedItem] {
        &self.items
    }
}

// This is manual because core `Name` values are deliberately not serializable;
// the backend owns their string representation in this interchange format.
impl Serialize for PlannedStyle {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut style = serializer.serialize_struct("PlannedStyle", 3)?;
        style.serialize_field("name", self.name.as_str())?;
        style.serialize_field("target", self.target.as_str())?;
        style.serialize_field("items", &self.items)?;
        style.end()
    }
}

/// One normalized item awaiting resolution by a running Godot engine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedItem {
    /// Native Godot theme-item name.
    property: Name,
    /// Source state that introduced the item, if any.
    state: Option<Name>,
    /// Native item categories that may accept the value.
    candidates: &'static [GodotItemKind],
    /// Source value category used in runtime diagnostics.
    value_kind: &'static str,
    /// Backend-normalized value sent to the Godot runtime.
    value: PreparedValue,
}

impl PlannedItem {
    /// Returns the exact native theme-item name.
    #[must_use]
    pub const fn property(&self) -> &Name {
        &self.property
    }

    /// Returns the source state, when this is a state-specific item.
    #[must_use]
    pub const fn state(&self) -> Option<&Name> {
        self.state.as_ref()
    }

    /// Returns possible native categories, to be resolved by Godot.
    #[must_use]
    pub const fn candidates(&self) -> &[GodotItemKind] {
        self.candidates
    }

    /// Returns the normalized portable value.
    #[must_use]
    pub const fn value(&self) -> &PreparedValue {
        &self.value
    }
}

// This is manual because core `Name` values and candidate categories need their
// backend-defined string representations rather than their Rust structure.
impl Serialize for PlannedItem {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let candidates = self
            .candidates
            .iter()
            .map(|candidate| candidate.wire_name())
            .collect::<Vec<_>>();
        let mut item = serializer.serialize_struct("PlannedItem", 5)?;
        item.serialize_field("property", self.property.as_str())?;
        item.serialize_field("state", &self.state.as_ref().map(Name::as_str))?;
        item.serialize_field("value_kind", self.value_kind)?;
        item.serialize_field("candidates", &candidates)?;
        item.serialize_field("value", &self.value)?;
        item.end()
    }
}

/// A value that satisfies Godot's portable theme-item constraints.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreparedValue {
    /// An sRGB color.
    Color(Color),
    /// A whole number of pixels within Godot's signed 32-bit range.
    Integer(i32),
    /// A reference in Godot's project or UID resource namespace.
    Resource(ResourceRef),
}

// This is manual because each variant has a distinct wire payload field
// (`rgba`, `value`, or `path`) derived from non-serializable core domain types.
impl Serialize for PreparedValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Color(color) => {
                let [red, green, blue] = color.components();
                let rgba = [red.get(), green.get(), blue.get(), color.alpha().get()];
                let mut value = serializer.serialize_struct("PreparedColor", 2)?;
                value.serialize_field("kind", "color")?;
                value.serialize_field("rgba", &rgba)?;
                value.end()
            }
            Self::Integer(integer) => {
                let mut value = serializer.serialize_struct("PreparedInteger", 2)?;
                value.serialize_field("kind", "integer")?;
                value.serialize_field("value", integer)?;
                value.end()
            }
            Self::Resource(reference) => {
                let mut value = serializer.serialize_struct("PreparedResource", 2)?;
                value.serialize_field("kind", "resource")?;
                value.serialize_field("path", reference.as_str())?;
                value.end()
            }
        }
    }
}

/// Candidate native Godot `Theme` item category resolved by a running engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GodotItemKind {
    /// A color item.
    Color,
    /// An integer constant.
    Constant,
    /// A positive font size.
    FontSize,
    /// A font resource.
    Font,
    /// A texture resource.
    Icon,
    /// A stylebox resource.
    StyleBox,
}

impl GodotItemKind {
    /// Returns the category spelling consumed by the Godot runtime builder.
    const fn wire_name(self) -> &'static str {
        match self {
            Self::Color => "color",
            Self::Constant => "constant",
            Self::FontSize => "font_size",
            Self::Font => "font",
            Self::Icon => "icon",
            Self::StyleBox => "stylebox",
        }
    }
}

/// Validates and normalizes a compiled theme without consulting Godot metadata.
///
/// The returned plan is suitable for transport to a running Godot engine,
/// which resolves its candidate item categories and constructs the resource.
pub fn plan_theme(compiled: &CompiledTheme) -> Result<GodotBuildPlan, BackendErrors> {
    let (plan, errors) = normalize_theme(compiled);
    if errors.is_empty() {
        Ok(plan)
    } else {
        Err(BackendErrors::new(errors))
    }
}

/// Normalizes every style while retaining all independent backend errors.
fn normalize_theme(compiled: &CompiledTheme) -> (GodotBuildPlan, Vec<BackendError>) {
    let mut errors = Vec::new();
    let styles = compiled
        .styles()
        .values()
        .map(|style| {
            let normalized = PlannedStyle {
                name: style.name().clone(),
                target: style.target().clone(),
                items: normalize_style(style, &mut errors),
            };
            (style.name().clone(), normalized)
        })
        .collect();
    (
        GodotBuildPlan {
            name: compiled.name().clone(),
            styles,
        },
        errors,
    )
}

/// Normalizes a style and rejects state entries that override a base item.
fn normalize_style(style: &CompiledStyle, errors: &mut Vec<BackendError>) -> Vec<PlannedItem> {
    let mut items = Vec::new();
    for (property, value) in style.properties() {
        normalize_item(style, None, property, value, &mut items, errors);
    }
    for state in style.states().values() {
        for (property, value) in state.properties() {
            let Some(base) = style.properties().get(property) else {
                normalize_item(
                    style,
                    Some(state.name()),
                    property,
                    value,
                    &mut items,
                    errors,
                );
                continue;
            };
            if base != value {
                errors.push(BackendError::StateOverridesBaseItem {
                    style: style.name().clone(),
                    state: state.name().clone(),
                    property: property.clone(),
                });
            }
        }
    }
    items
}

/// Appends one normalized item or records its validation error.
fn normalize_item(
    style: &CompiledStyle,
    state: Option<&Name>,
    property: &Name,
    value: &CompiledValue,
    items: &mut Vec<PlannedItem>,
    errors: &mut Vec<BackendError>,
) {
    match normalize_value(style, state, property, value) {
        Ok((candidates, prepared_value)) => items.push(PlannedItem {
            property: property.clone(),
            state: state.cloned(),
            candidates,
            value_kind: value_kind(value),
            value: prepared_value,
        }),
        Err(error) => errors.push(error),
    }
}

/// Converts a compiled value and lists the native categories that may accept it.
fn normalize_value(
    style: &CompiledStyle,
    state: Option<&Name>,
    property: &Name,
    value: &CompiledValue,
) -> Result<(&'static [GodotItemKind], PreparedValue), BackendError> {
    match value {
        CompiledValue::Color(color) => Ok((
            &[GodotItemKind::Color, GodotItemKind::StyleBox],
            PreparedValue::Color(*color),
        )),
        CompiledValue::Dimension(_) | CompiledValue::Number(_) => {
            let integer = integral_pixels(value).ok_or_else(|| BackendError::InvalidInteger {
                style: style.name().clone(),
                property: property.clone(),
                expected: "a whole number of pixels",
            })?;
            Ok((
                &[GodotItemKind::Constant, GodotItemKind::FontSize],
                PreparedValue::Integer(integer),
            ))
        }
        CompiledValue::Resource(reference) if valid_resource_reference(reference) => Ok((
            &[
                GodotItemKind::Font,
                GodotItemKind::Icon,
                GodotItemKind::StyleBox,
            ],
            PreparedValue::Resource(reference.clone()),
        )),
        CompiledValue::Resource(reference) => Err(BackendError::InvalidResourceReference {
            reference: reference.clone(),
        }),
        CompiledValue::Boolean(_) | CompiledValue::String(_) => {
            Err(BackendError::UnsupportedValue {
                style: style.name().clone(),
                target: style.target().clone(),
                state: state.cloned(),
                property: property.clone(),
                value: value_kind(value),
            })
        }
    }
}

/// Returns whether a resource uses a non-empty Godot project or UID namespace.
fn valid_resource_reference(reference: &ResourceRef) -> bool {
    let value = reference.as_str();
    (value.starts_with("res://") && value.len() > "res://".len())
        || (value.starts_with("uid://") && value.len() > "uid://".len())
}

/// Extracts an integral, signed 32-bit pixel count from a compatible value.
fn integral_pixels(value: &CompiledValue) -> Option<i32> {
    let number = match value {
        CompiledValue::Number(number) => number.get(),
        CompiledValue::Dimension(dimension) if dimension.unit() == DimensionUnit::Pixel => {
            dimension.value().get()
        }
        _ => return None,
    };
    if number.fract() != 0.0 || number < f64::from(i32::MIN) || number > f64::from(i32::MAX) {
        return None;
    }
    Some(number as i32)
}

/// Returns the stable source value category used in backend diagnostics.
fn value_kind(value: &CompiledValue) -> &'static str {
    match value {
        CompiledValue::Boolean(_) => "boolean",
        CompiledValue::Color(_) => "color",
        CompiledValue::Dimension(_) => "dimension",
        CompiledValue::Number(_) => "number",
        CompiledValue::String(_) => "string",
        CompiledValue::Resource(_) => "resource",
    }
}
