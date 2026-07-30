use std::str::FromStr;

use kdl::{KdlDocument, KdlNode};
use themosis_core::{
    Name, Number, PropertyAssignment, ResourceRef, SourceId, StyleDefinition, StyleDocument,
    StyleState, StyleValue, TokenPath,
};

use crate::{
    decode::{DecodeValue, Decoded, Decoder},
    error::{ParseError, ParseErrors, StructureError, SyntaxError},
};

/// Parses one component-style KDL 2 source.
pub fn parse(file_name: &str, source: SourceId, input: &str) -> Result<StyleDocument, ParseErrors> {
    let parsed = KdlDocument::parse_v2(input).map_err(|error| {
        ParseErrors::one(ParseError::Syntax(SyntaxError::new(file_name, error)))
    })?;

    if parsed.nodes().len() != 1 {
        return Err(ParseErrors::one(ParseError::Structure(
            StructureError::new("document", "expected exactly one theme node"),
        )));
    }

    let root = &parsed.nodes()[0];
    if root.name().value() != "theme" {
        let decoder = Decoder::new(source);
        let span = decoder.node_name_span(root);
        return Err(ParseErrors::one(ParseError::Structure(StructureError::at(
            "document",
            format!("expected theme node, found '{}'", root.name().value()),
            span,
        ))));
    }

    let mut converter = Converter::new(source);
    let document = converter.convert_theme(root);
    let errors = converter.into_errors();

    if errors.is_empty() {
        document.ok_or_else(|| {
            ParseErrors::one(ParseError::Structure(StructureError::new(
                "theme",
                "theme could not be converted",
            )))
        })
    } else {
        Err(ParseErrors::new(
            errors.into_iter().map(ParseError::Structure).collect(),
        ))
    }
}

struct Converter {
    decoder: Decoder,
}

impl Converter {
    const fn new(source: SourceId) -> Self {
        Self {
            decoder: Decoder::new(source),
        }
    }

    fn into_errors(self) -> Vec<StructureError> {
        self.decoder.into_errors()
    }

    fn convert_theme(&mut self, node: &KdlNode) -> Option<StyleDocument> {
        let (raw_name, children) = {
            let mut raw = self.decoder.node(node, "theme");
            let name = raw.required_argument::<String>(0, "name");
            let children = raw.children();
            raw.finish();
            (name, children)
        };
        let name = raw_name.and_then(|value| self.name("theme", value));
        let mut token_sources = Vec::new();
        let mut imports = Vec::new();
        let mut styles = Vec::new();

        for child in children {
            match child.name().value() {
                "tokens" => {
                    if let Some(path) = self.convert_source_path("tokens", child) {
                        token_sources.push(path);
                    }
                }
                "import" => {
                    if let Some(path) = self.convert_source_path("import", child) {
                        imports.push(path);
                    }
                }
                "style" => {
                    if let Some(style) = self.convert_style(child) {
                        styles.push(style);
                    }
                }
                _ => self.decoder.unexpected_node("theme", child),
            }
        }

        Some(StyleDocument::new(
            self.decoder.source(),
            name?,
            token_sources,
            imports,
            styles,
        ))
    }

    fn convert_source_path(&mut self, context: &str, node: &KdlNode) -> Option<String> {
        let raw_path = {
            let mut raw = self.decoder.node(node, context);
            let path = raw.required_argument::<String>(0, "path");
            raw.finish();
            path
        };
        self.source_path(context, raw_path?)
    }

    fn convert_style(&mut self, node: &KdlNode) -> Option<StyleDefinition> {
        let (span, raw_name, raw_target, raw_extends, children) = {
            let mut raw = self.decoder.node(node, "style");
            let span = raw.span();
            let name = raw.required_argument::<String>(0, "name");
            let target = raw.required_property::<String>("target");
            let extends = raw.optional_property::<String>("extends");
            let children = raw.children();
            raw.finish();
            (span, name, target, extends, children)
        };
        let context = raw_name.as_ref().map_or_else(
            || "style".to_owned(),
            |name| format!("style '{}'", name.value()),
        );
        let name = raw_name.and_then(|value| self.name(&context, value));
        let target = raw_target.and_then(|value| self.name(&format!("{context} target"), value));
        let extends = raw_extends.and_then(|value| self.name(&format!("{context} extends"), value));
        let mut properties = Vec::new();
        let mut states = Vec::new();

        for child in children {
            match child.name().value() {
                "state" => {
                    if let Some(state) = self.convert_state(&context, child) {
                        states.push(state);
                    }
                }
                "boolean" => {
                    Self::push_property(&mut properties, self.convert_boolean(&context, child))
                }
                "number" => {
                    Self::push_property(&mut properties, self.convert_number(&context, child))
                }
                "string" => {
                    Self::push_property(&mut properties, self.convert_string(&context, child))
                }
                "token" => {
                    Self::push_property(&mut properties, self.convert_token(&context, child))
                }
                "resource" => {
                    Self::push_property(&mut properties, self.convert_resource(&context, child))
                }
                _ => self.decoder.unexpected_node(&context, child),
            }
        }

        Some(StyleDefinition::spanned(
            name?, target?, extends, properties, states, span,
        ))
    }

    fn convert_state(&mut self, style_context: &str, node: &KdlNode) -> Option<StyleState> {
        let (span, raw_name, children) = {
            let mut raw = self.decoder.node(node, format!("{style_context} state"));
            let span = raw.span();
            let name = raw.required_argument::<String>(0, "name");
            let children = raw.children();
            raw.finish();
            (span, name, children)
        };
        let context = raw_name.as_ref().map_or_else(
            || format!("{style_context} state"),
            |name| format!("{style_context} state '{}'", name.value()),
        );
        let name = raw_name.and_then(|value| self.name(&context, value));
        let mut properties = Vec::new();

        for child in children {
            let property = match child.name().value() {
                "boolean" => self.convert_boolean(&context, child),
                "number" => self.convert_number(&context, child),
                "string" => self.convert_string(&context, child),
                "token" => self.convert_token(&context, child),
                "resource" => self.convert_resource(&context, child),
                _ => {
                    self.decoder.unexpected_node(&context, child);
                    None
                }
            };
            Self::push_property(&mut properties, property);
        }

        Some(StyleState::spanned(name?, properties, span))
    }

    fn convert_boolean(&mut self, context: &str, node: &KdlNode) -> Option<PropertyAssignment> {
        let (span, raw_name, raw_value) = self.decode_property::<bool>(context, node);
        let property_context = self.property_context(context, raw_name.as_ref());
        let name = raw_name.and_then(|value| self.name(&property_context, value));
        let value = raw_value.map(Decoded::into_parts).map(|(value, _)| value);
        Some(PropertyAssignment::spanned(
            name?,
            StyleValue::Boolean(value?),
            span,
        ))
    }

    fn convert_number(&mut self, context: &str, node: &KdlNode) -> Option<PropertyAssignment> {
        let (span, raw_name, raw_value) = self.decode_property::<f64>(context, node);
        let property_context = self.property_context(context, raw_name.as_ref());
        let name = raw_name.and_then(|value| self.name(&property_context, value));
        let value = raw_value.and_then(|value| {
            let (value, value_span) = value.into_parts();
            match Number::new(value) {
                Ok(value) => Some(value),
                Err(error) => {
                    self.decoder
                        .error_at(&property_context, error.to_string(), value_span);
                    None
                }
            }
        });
        Some(PropertyAssignment::spanned(
            name?,
            StyleValue::Number(value?),
            span,
        ))
    }

    fn convert_string(&mut self, context: &str, node: &KdlNode) -> Option<PropertyAssignment> {
        let (span, raw_name, raw_value) = self.decode_property::<String>(context, node);
        let property_context = self.property_context(context, raw_name.as_ref());
        let name = raw_name.and_then(|value| self.name(&property_context, value));
        let value = raw_value.map(Decoded::into_parts).map(|(value, _)| value);
        Some(PropertyAssignment::spanned(
            name?,
            StyleValue::String(value?),
            span,
        ))
    }

    fn convert_token(&mut self, context: &str, node: &KdlNode) -> Option<PropertyAssignment> {
        let (span, raw_name, raw_value) = self.decode_property::<String>(context, node);
        let property_context = self.property_context(context, raw_name.as_ref());
        let name = raw_name.and_then(|value| self.name(&property_context, value));
        let path = raw_value.and_then(|value| {
            let (value, value_span) = value.into_parts();
            match TokenPath::from_str(&value) {
                Ok(path) => Some(path),
                Err(error) => {
                    self.decoder
                        .error_at(&property_context, error.to_string(), value_span);
                    None
                }
            }
        });
        Some(PropertyAssignment::spanned(
            name?,
            StyleValue::Token(path?),
            span,
        ))
    }

    fn convert_resource(&mut self, context: &str, node: &KdlNode) -> Option<PropertyAssignment> {
        let (span, raw_name, raw_value) = self.decode_property::<String>(context, node);
        let property_context = self.property_context(context, raw_name.as_ref());
        let name = raw_name.and_then(|value| self.name(&property_context, value));
        let reference = raw_value.and_then(|value| {
            let (value, value_span) = value.into_parts();
            match ResourceRef::new(value) {
                Ok(reference) => Some(reference),
                Err(error) => {
                    self.decoder
                        .error_at(&property_context, error.to_string(), value_span);
                    None
                }
            }
        });
        Some(PropertyAssignment::spanned(
            name?,
            StyleValue::Resource(reference?),
            span,
        ))
    }

    fn decode_property<T: DecodeValue>(
        &mut self,
        context: &str,
        node: &KdlNode,
    ) -> (
        themosis_core::Span,
        Option<Decoded<String>>,
        Option<Decoded<T>>,
    ) {
        let mut raw = self.decoder.node(node, format!("{context} property"));
        let span = raw.span();
        let name = raw.required_argument::<String>(0, "name");
        let value = raw.required_argument::<T>(1, "value");
        raw.finish();
        (span, name, value)
    }

    fn name(&mut self, context: &str, value: Decoded<String>) -> Option<Name> {
        let (value, span) = value.into_parts();
        match Name::new(value) {
            Ok(name) => Some(name),
            Err(error) => {
                self.decoder.error_at(context, error.to_string(), span);
                None
            }
        }
    }

    fn source_path(&mut self, context: &str, value: Decoded<String>) -> Option<String> {
        let (value, span) = value.into_parts();
        if value.is_empty() || value.trim() != value {
            self.decoder.error_at(
                context,
                "source path must be non-empty and have no surrounding whitespace",
                span,
            );
            None
        } else {
            Some(value)
        }
    }

    fn property_context(&self, context: &str, name: Option<&Decoded<String>>) -> String {
        name.map_or_else(
            || format!("{context} property"),
            |name| format!("{context} property '{}'", name.value()),
        )
    }

    fn push_property(
        properties: &mut Vec<PropertyAssignment>,
        property: Option<PropertyAssignment>,
    ) {
        if let Some(property) = property {
            properties.push(property);
        }
    }
}

#[cfg(test)]
mod tests {
    use themosis_core::{Diagnostic, SourceId, StyleValue};

    use super::{ParseError, ParseErrors, StructureError, parse};

    const VALID: &str = include_str!("../tests/fixtures/valid/theme.kdl");

    fn structure_errors(errors: &ParseErrors) -> Vec<&StructureError> {
        errors
            .errors()
            .iter()
            .map(|error| match error {
                ParseError::Structure(error) => error,
                ParseError::Syntax(_) => panic!("expected structure error"),
            })
            .collect()
    }

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
            style.properties()[2].value(),
            StyleValue::Boolean(false)
        ));
        assert!(matches!(
            style.properties()[4].value(),
            StyleValue::Resource(reference) if reference.as_str() == "res://fonts/ui.tres"
        ));
        assert_eq!(style.states()[0].name().as_str(), "hover");
    }

    #[test]
    fn accepts_kdl_2_bare_strings_and_hash_booleans() {
        let input = "theme dark { style Primary target=Button { boolean disabled #false } }\n";
        let document = parse("v2.kdl", SourceId::new(0), input).expect("input is valid");

        assert_eq!(document.name().as_str(), "dark");
        assert!(matches!(
            document.styles()[0].properties()[0].value(),
            StyleValue::Boolean(false)
        ));
    }

    #[test]
    fn rejects_kdl_1_boolean_syntax() {
        let input = "theme dark { style Primary target=Button { boolean disabled false } }\n";
        let error = parse("v1.kdl", SourceId::new(0), input).expect_err("v1 is not accepted");

        assert_eq!(error.errors()[0].code(), "TMS1001");
        assert!(matches!(error.errors()[0], ParseError::Syntax(_)));
        assert!(error.to_string().starts_with("error[TMS1001]: v1.kdl:"));
    }

    #[test]
    fn collects_schema_errors() {
        let input = include_str!("../tests/fixtures/invalid/schema.kdl");
        let error = parse("schema.kdl", SourceId::new(0), input).expect_err("fixture is invalid");
        assert_eq!(error.errors()[0].code(), "TMS1002");
        let messages = structure_errors(&error)
            .into_iter()
            .map(StructureError::message)
            .collect::<Vec<_>>();

        assert!(messages.contains(&"property 'target' is required"));
        assert!(messages.contains(&"unexpected node 'unknown'"));
        assert_eq!(
            error.to_string().matches("error[TMS1002]:").count(),
            error.len()
        );
    }

    #[test]
    fn collects_core_structure_errors() {
        let input = include_str!("../tests/fixtures/invalid/values.kdl");
        let error = parse("values.kdl", SourceId::new(0), input).expect_err("fixture is invalid");
        let messages = structure_errors(&error)
            .into_iter()
            .map(StructureError::message)
            .collect::<Vec<_>>();

        assert!(messages.contains(&"token path segment 1 is empty"));
        assert!(messages.contains(&"resource reference cannot start or end with whitespace"));
    }

    #[test]
    fn reports_duplicates_unknown_properties_and_extra_arguments() {
        let input = r#"
theme dark {
    style Primary extra target=Button target=Other unknown=value
}
"#;
        let error = parse("schema.kdl", SourceId::new(0), input).expect_err("input is invalid");
        let messages = structure_errors(&error)
            .into_iter()
            .map(StructureError::message)
            .collect::<Vec<_>>();

        assert!(messages.contains(&"duplicate property 'target'"));
        assert!(messages.contains(&"unexpected argument"));
        assert!(messages.contains(&"unexpected property 'unknown'"));
    }

    #[test]
    fn schema_type_errors_use_entry_spans() {
        let input = "theme dark { style Primary target=#true }\n";
        let error = parse("span.kdl", SourceId::new(6), input).expect_err("target is not a string");
        let span = structure_errors(&error)[0]
            .span()
            .expect("type error is spanned");

        assert_eq!(span.source(), SourceId::new(6));
        assert_eq!(&input[span.range()], "target=#true");
    }

    #[test]
    fn rejects_non_finite_numbers() {
        let input = "theme dark { style Primary target=Button { number opacity #inf } }\n";
        let error = parse("number.kdl", SourceId::new(0), input).expect_err("infinity is invalid");
        assert!(structure_errors(&error)[0].message().contains("finite"));
    }

    #[test]
    fn requires_exactly_one_theme_root() {
        let error = parse("empty.kdl", SourceId::new(0), "").expect_err("root is absent");

        assert!(error.to_string().contains("exactly one theme node"));
    }

    #[test]
    fn rejects_a_different_root_node() {
        let input = "styles dark\n";
        let error = parse("root.kdl", SourceId::new(0), input).expect_err("root name is wrong");

        assert!(error.to_string().contains("expected theme node"));
    }
}
