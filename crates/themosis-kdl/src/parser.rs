use std::str::FromStr;

use knus::ast::{Literal, Radix};
use themosis_core::{
    Name, Number, PropertyAssignment, ResourceRef, SourceId, StyleDefinition, StyleDocument,
    StyleState, StyleValue, TokenPath,
};
use thiserror::Error;

use crate::{
    error::{ParseError, StructureError, StructureErrors},
    raw::{
        RawBooleanProperty, RawNumberProperty, RawProperty, RawRoot, RawState, RawStringProperty,
        RawStyle, RawStyleChild, RawTheme, RawThemeChild,
    },
};

/// Parses one component-style KDL source.
pub fn parse(file_name: &str, source: SourceId, input: &str) -> Result<StyleDocument, ParseError> {
    let roots: Vec<RawRoot> =
        knus::parse(file_name, input).map_err(|error| ParseError::Decode(Box::new(error)))?;

    if roots.len() != 1 {
        return Err(ParseError::Structure(StructureErrors(vec![
            StructureError::new("document", "expected exactly one theme node"),
        ])));
    }

    let RawRoot::Theme(theme) = roots
        .into_iter()
        .next()
        .expect("length was checked before extracting the root");
    let mut converter = Converter::new(source);
    let document = converter.convert_theme(theme);

    if converter.errors.is_empty() {
        document.ok_or_else(|| {
            ParseError::Structure(StructureErrors(vec![StructureError::new(
                "theme",
                "theme could not be converted",
            )]))
        })
    } else {
        Err(ParseError::Structure(StructureErrors(converter.errors)))
    }
}

struct Converter {
    source: SourceId,
    errors: Vec<StructureError>,
}

impl Converter {
    fn new(source: SourceId) -> Self {
        Self {
            source,
            errors: Vec::new(),
        }
    }

    fn span(&self, span: knus::span::Span) -> themosis_core::Span {
        themosis_core::Span::new(self.source, span.0..span.1)
            .expect("knus always returns ordered source spans")
    }

    fn convert_theme(&mut self, raw: RawTheme) -> Option<StyleDocument> {
        let theme_span = self.span(raw.span);
        let name = self.name("theme", raw.name, theme_span);
        let mut token_sources = Vec::new();
        let mut imports = Vec::new();
        let mut styles = Vec::new();

        for child in raw.children {
            match child {
                RawThemeChild::Tokens(raw) => {
                    let span = self.span(raw.span);
                    if let Some(path) = self.source_path("tokens", raw.path, span) {
                        token_sources.push(path);
                    }
                }
                RawThemeChild::Import(raw) => {
                    let span = self.span(raw.span);
                    if let Some(path) = self.source_path("import", raw.path, span) {
                        imports.push(path);
                    }
                }
                RawThemeChild::Style(raw) => {
                    if let Some(style) = self.convert_style(raw) {
                        styles.push(style);
                    }
                }
            }
        }

        Some(StyleDocument::new(
            self.source,
            name?,
            token_sources,
            imports,
            styles,
        ))
    }

    fn convert_style(&mut self, raw: RawStyle) -> Option<StyleDefinition> {
        let span = self.span(raw.span);
        let context = format!("style '{}'", raw.name);
        let name = self.name(&context, raw.name, span);
        let target = self.name(&format!("{context} target"), raw.target, span);
        let extends = raw
            .extends
            .and_then(|value| self.name(&format!("{context} extends"), value, span));
        let mut properties = Vec::new();
        let mut states = Vec::new();

        for child in raw.children {
            match child {
                RawStyleChild::State(raw) => {
                    if let Some(state) = self.convert_state(&context, raw) {
                        states.push(state);
                    }
                }
                RawStyleChild::Boolean(raw) => {
                    if let Some(property) = self.convert_boolean(&context, raw) {
                        properties.push(property);
                    }
                }
                RawStyleChild::Number(raw) => {
                    if let Some(property) = self.convert_number(&context, raw) {
                        properties.push(property);
                    }
                }
                RawStyleChild::String(raw) => {
                    if let Some(property) = self.convert_string(&context, raw) {
                        properties.push(property);
                    }
                }
                RawStyleChild::Token(raw) => {
                    if let Some(property) = self.convert_token(&context, raw) {
                        properties.push(property);
                    }
                }
                RawStyleChild::Resource(raw) => {
                    if let Some(property) = self.convert_resource(&context, raw) {
                        properties.push(property);
                    }
                }
            }
        }

        Some(StyleDefinition::spanned(
            name?, target?, extends, properties, states, span,
        ))
    }

    fn convert_state(&mut self, style_context: &str, raw: RawState) -> Option<StyleState> {
        let span = self.span(raw.span);
        let context = format!("{style_context} state '{}'", raw.name);
        let name = self.name(&context, raw.name, span);
        let mut properties = Vec::new();

        for property in raw.properties {
            let property = match property {
                RawProperty::Boolean(raw) => self.convert_boolean(&context, raw),
                RawProperty::Number(raw) => self.convert_number(&context, raw),
                RawProperty::String(raw) => self.convert_string(&context, raw),
                RawProperty::Token(raw) => self.convert_token(&context, raw),
                RawProperty::Resource(raw) => self.convert_resource(&context, raw),
            };
            if let Some(property) = property {
                properties.push(property);
            }
        }

        Some(StyleState::spanned(name?, properties, span))
    }

    fn convert_boolean(
        &mut self,
        context: &str,
        raw: RawBooleanProperty,
    ) -> Option<PropertyAssignment> {
        let span = self.span(raw.span);
        let name = self.name(&format!("{context} property"), raw.name, span)?;
        Some(PropertyAssignment::spanned(
            name,
            StyleValue::Boolean(raw.value),
            span,
        ))
    }

    fn convert_number(
        &mut self,
        context: &str,
        raw: RawNumberProperty,
    ) -> Option<PropertyAssignment> {
        let span = self.span(raw.span);
        let property_context = format!("{context} property '{}'", raw.name);
        let name = self.name(&property_context, raw.name, span)?;
        let raw_value = match literal_number(&raw.value) {
            Ok(value) => value,
            Err(error) => {
                self.errors.push(StructureError::at(
                    property_context,
                    error.to_string(),
                    span,
                ));
                return None;
            }
        };
        let value = match Number::new(raw_value) {
            Ok(value) => value,
            Err(error) => {
                self.errors.push(StructureError::at(
                    property_context,
                    error.to_string(),
                    span,
                ));
                return None;
            }
        };
        Some(PropertyAssignment::spanned(
            name,
            StyleValue::Number(value),
            span,
        ))
    }

    fn convert_string(
        &mut self,
        context: &str,
        raw: RawStringProperty,
    ) -> Option<PropertyAssignment> {
        let span = self.span(raw.span);
        let name = self.name(&format!("{context} property"), raw.name, span)?;
        Some(PropertyAssignment::spanned(
            name,
            StyleValue::String(raw.value),
            span,
        ))
    }

    fn convert_token(
        &mut self,
        context: &str,
        raw: RawStringProperty,
    ) -> Option<PropertyAssignment> {
        let span = self.span(raw.span);
        let property_context = format!("{context} property '{}'", raw.name);
        let name = self.name(&property_context, raw.name, span)?;
        let path = match TokenPath::from_str(&raw.value) {
            Ok(path) => path,
            Err(error) => {
                self.errors.push(StructureError::at(
                    property_context,
                    error.to_string(),
                    span,
                ));
                return None;
            }
        };
        Some(PropertyAssignment::spanned(
            name,
            StyleValue::Token(path),
            span,
        ))
    }

    fn convert_resource(
        &mut self,
        context: &str,
        raw: RawStringProperty,
    ) -> Option<PropertyAssignment> {
        let span = self.span(raw.span);
        let property_context = format!("{context} property '{}'", raw.name);
        let name = self.name(&property_context, raw.name, span)?;
        let reference = match ResourceRef::new(raw.value) {
            Ok(reference) => reference,
            Err(error) => {
                self.errors.push(StructureError::at(
                    property_context,
                    error.to_string(),
                    span,
                ));
                return None;
            }
        };
        Some(PropertyAssignment::spanned(
            name,
            StyleValue::Resource(reference),
            span,
        ))
    }

    fn name(&mut self, context: &str, value: String, span: themosis_core::Span) -> Option<Name> {
        match Name::new(value) {
            Ok(name) => Some(name),
            Err(error) => {
                self.errors
                    .push(StructureError::at(context, error.to_string(), span));
                None
            }
        }
    }

    fn source_path(
        &mut self,
        context: &str,
        value: String,
        span: themosis_core::Span,
    ) -> Option<String> {
        if value.is_empty() || value.trim() != value {
            self.errors.push(StructureError::at(
                context,
                "source path must be non-empty and have no surrounding whitespace",
                span,
            ));
            None
        } else {
            Some(value)
        }
    }
}

fn literal_number(value: &Literal) -> Result<f64, NumberLiteralError> {
    match value {
        Literal::Int(value) => match value.0 {
            Radix::Dec => value
                .1
                .parse::<f64>()
                .map_err(|_| NumberLiteralError::OutOfRange),
            Radix::Bin | Radix::Oct | Radix::Hex => {
                let radix = match value.0 {
                    Radix::Bin => 2,
                    Radix::Oct => 8,
                    Radix::Hex => 16,
                    Radix::Dec => unreachable!("decimal radix handled above"),
                };
                i64::from_str_radix(&value.1, radix)
                    .map(|value| value as f64)
                    .map_err(|_| NumberLiteralError::OutOfRange)
            }
        },
        Literal::Decimal(value) => value
            .0
            .parse::<f64>()
            .map_err(|_| NumberLiteralError::OutOfRange),
        Literal::Null | Literal::Bool(_) | Literal::String(_) => Err(NumberLiteralError::WrongType),
    }
}

#[derive(Clone, Copy, Debug, Error)]
enum NumberLiteralError {
    #[error("number property value must be numeric")]
    WrongType,
    #[error("number property value is out of range")]
    OutOfRange,
}

#[cfg(test)]
mod tests {
    use themosis_core::{SourceId, StyleValue};

    use super::{ParseError, parse};

    const VALID: &str = include_str!("../tests/fixtures/valid/theme.kdl");

    #[test]
    fn decodes_theme_sources_styles_and_states() {
        let document = parse("theme.kdl", SourceId::new(4), VALID).expect("fixture is valid");
        let style = &document.styles()[0];

        assert_eq!(document.name().as_str(), "dark");
        assert_eq!(document.token_sources(), ["tokens/dark.tokens.json"]);
        assert_eq!(document.imports(), ["controls.kdl"]);
        assert_eq!(style.name().as_str(), "PrimaryButton");
        assert_eq!(style.target().as_str(), "Button");
        assert_eq!(
            style.extends().map(themosis_core::Name::as_str),
            Some("BaseButton")
        );
        assert!(matches!(
            style.properties()[0].value(),
            StyleValue::Token(path) if path.to_string() == "color.primary"
        ));
        assert!(matches!(
            style.properties()[1].value(),
            StyleValue::Number(value) if value.get() == 16.0
        ));
        assert!(matches!(
            style.properties()[4].value(),
            StyleValue::Resource(reference) if reference.as_str() == "res://fonts/ui.tres"
        ));
        assert_eq!(style.states()[0].name().as_str(), "hover");
    }

    #[test]
    fn knus_reports_schema_errors() {
        let input = include_str!("../tests/fixtures/invalid/schema.kdl");
        let error = parse("schema.kdl", SourceId::new(0), input).expect_err("fixture is invalid");

        assert!(matches!(error, ParseError::Decode(_)));
    }

    #[test]
    fn collects_core_structure_errors() {
        let input = include_str!("../tests/fixtures/invalid/values.kdl");
        let error = parse("values.kdl", SourceId::new(0), input).expect_err("fixture is invalid");
        let ParseError::Structure(errors) = error else {
            panic!("expected structure errors");
        };
        let messages: Vec<&str> = errors
            .errors()
            .iter()
            .map(super::StructureError::message)
            .collect();

        assert!(messages.contains(&"token path segment 1 is empty"));
        assert!(messages.contains(&"resource reference cannot start or end with whitespace"));
    }

    #[test]
    fn requires_exactly_one_theme_root() {
        let error = parse("empty.kdl", SourceId::new(0), "").expect_err("root is absent");

        assert!(error.to_string().contains("exactly one theme node"));
    }
}
