use std::str::FromStr;

use serde_json::{Map, Value};
use themosis_core::{
    Color, Dimension, DimensionUnit, Number, SourceId, TokenDefinition, TokenDocument,
    TokenExpression, TokenKind, TokenPath, TokenValue,
};

use crate::{ParseError, ParseErrors};

/// Parses one token JSON source into an unresolved token document.
pub fn parse(source: SourceId, input: &str) -> Result<TokenDocument, ParseErrors> {
    let root: Value = serde_json::from_str(input).map_err(|error| {
        ParseErrors::one(ParseError::syntax(
            error.line(),
            error.column(),
            error.to_string(),
        ))
    })?;

    let Some(root) = root.as_object() else {
        return Err(ParseErrors::one(ParseError::at(
            "$",
            "token document root must be an object",
        )));
    };

    let mut parser = Parser::default();
    parser.visit_group(root, &mut Vec::new(), None);

    if parser.errors.is_empty() {
        Ok(TokenDocument::new(source, parser.tokens))
    } else {
        Err(ParseErrors::new(parser.errors))
    }
}

#[derive(Default)]
struct Parser {
    tokens: Vec<TokenDefinition>,
    errors: Vec<ParseError>,
}

impl Parser {
    fn visit_group(
        &mut self,
        object: &Map<String, Value>,
        path: &mut Vec<String>,
        inherited_kind: Option<TokenKind>,
    ) {
        let location = json_path(path);
        self.validate_reserved_properties(object, &location, false);

        let local_kind = object
            .get("$type")
            .and_then(|value| self.parse_kind(value, &format!("{location}.$type")));
        let inherited_kind = local_kind.or(inherited_kind);

        for (name, value) in object.iter().filter(|(name, _)| !name.starts_with('$')) {
            if !valid_name(name) {
                self.errors.push(ParseError::at(
                    format!("{location}.{name}"),
                    "names must be non-empty and cannot contain '.', '{', or '}'",
                ));
                continue;
            }

            let child_location = format!("{location}.{name}");
            let Some(child) = value.as_object() else {
                self.errors.push(ParseError::at(
                    child_location,
                    "group entries must be objects",
                ));
                continue;
            };

            path.push(name.clone());
            if child.contains_key("$value") {
                self.visit_token(child, path, inherited_kind);
            } else {
                self.visit_group(child, path, inherited_kind);
            }
            path.pop();
        }
    }

    fn visit_token(
        &mut self,
        object: &Map<String, Value>,
        path: &[String],
        inherited_kind: Option<TokenKind>,
    ) {
        let location = json_path(path);
        self.validate_reserved_properties(object, &location, true);

        for name in object.keys().filter(|name| !name.starts_with('$')) {
            self.errors.push(ParseError::at(
                format!("{location}.{name}"),
                "tokens cannot contain child groups or tokens",
            ));
        }

        let local_kind = object
            .get("$type")
            .and_then(|value| self.parse_kind(value, &format!("{location}.$type")));
        let Some(kind) = local_kind.or(inherited_kind) else {
            self.errors.push(ParseError::at(
                &location,
                "token has no $type and does not inherit one",
            ));
            return;
        };

        let value_location = format!("{location}.$value");
        let Some(value) = object.get("$value") else {
            self.errors
                .push(ParseError::at(value_location, "token is missing $value"));
            return;
        };
        let Some(expression) = self.parse_expression(kind, value, &value_location) else {
            return;
        };

        let path = TokenPath::new(path.iter().cloned())
            .expect("JSON traversal only creates non-empty token paths");
        let definition = TokenDefinition::new(path, kind, expression)
            .expect("literal parser enforces declared token types");
        self.tokens.push(definition);
    }

    fn parse_kind(&mut self, value: &Value, location: &str) -> Option<TokenKind> {
        let Some(name) = value.as_str() else {
            self.errors
                .push(ParseError::at(location, "$type must be a string"));
            return None;
        };

        let kind = match name {
            "boolean" => TokenKind::Boolean,
            "color" => TokenKind::Color,
            "dimension" => TokenKind::Dimension,
            "number" => TokenKind::Number,
            "string" => TokenKind::String,
            _ => {
                self.errors.push(ParseError::at(
                    location,
                    format!("unsupported token type '{name}'"),
                ));
                return None;
            }
        };

        Some(kind)
    }

    fn parse_expression(
        &mut self,
        kind: TokenKind,
        value: &Value,
        location: &str,
    ) -> Option<TokenExpression> {
        if let Some(alias) = value.as_str().and_then(alias_path) {
            return match TokenPath::from_str(alias) {
                Ok(path) => Some(TokenExpression::Alias(path)),
                Err(error) => {
                    self.errors.push(ParseError::at(
                        location,
                        format!("invalid token alias: {error}"),
                    ));
                    None
                }
            };
        }

        let literal = match kind {
            TokenKind::Boolean => value.as_bool().map(TokenValue::Boolean).or_else(|| {
                self.expected(location, "a boolean");
                None
            }),
            TokenKind::Number => self.parse_number(value, location).map(TokenValue::Number),
            TokenKind::String => value
                .as_str()
                .map(|value| TokenValue::String(value.to_owned()))
                .or_else(|| {
                    self.expected(location, "a string");
                    None
                }),
            TokenKind::Color => self.parse_color(value, location).map(TokenValue::Color),
            TokenKind::Dimension => self
                .parse_dimension(value, location)
                .map(TokenValue::Dimension),
        }?;

        Some(TokenExpression::Literal(literal))
    }

    fn parse_number(&mut self, value: &Value, location: &str) -> Option<Number> {
        let Some(value) = value.as_f64() else {
            self.expected(location, "a number");
            return None;
        };

        match Number::new(value) {
            Ok(value) => Some(value),
            Err(error) => {
                self.errors
                    .push(ParseError::at(location, error.to_string()));
                None
            }
        }
    }

    fn parse_color(&mut self, value: &Value, location: &str) -> Option<Color> {
        let Some(object) = value.as_object() else {
            self.expected(location, "a color object");
            return None;
        };
        if !self.validate_object_keys(object, location, &["colorSpace", "components", "alpha"]) {
            return None;
        }

        match object.get("colorSpace").and_then(Value::as_str) {
            Some("srgb") => {}
            Some(space) => {
                self.errors.push(ParseError::at(
                    format!("{location}.colorSpace"),
                    format!("unsupported color space '{space}'"),
                ));
                return None;
            }
            None => {
                self.expected(&format!("{location}.colorSpace"), "the string 'srgb'");
                return None;
            }
        }

        let Some(components) = object.get("components").and_then(Value::as_array) else {
            self.expected(
                &format!("{location}.components"),
                "an array of three numbers",
            );
            return None;
        };
        if components.len() != 3 {
            self.expected(
                &format!("{location}.components"),
                "an array of three numbers",
            );
            return None;
        }
        let Some(red) = components[0].as_f64() else {
            self.expected(&format!("{location}.components[0]"), "a number");
            return None;
        };
        let Some(green) = components[1].as_f64() else {
            self.expected(&format!("{location}.components[1]"), "a number");
            return None;
        };
        let Some(blue) = components[2].as_f64() else {
            self.expected(&format!("{location}.components[2]"), "a number");
            return None;
        };
        let Some(alpha) = object.get("alpha").and_then(Value::as_f64) else {
            self.expected(&format!("{location}.alpha"), "a number");
            return None;
        };

        match Color::new([red, green, blue], alpha) {
            Ok(color) => Some(color),
            Err(error) => {
                self.errors
                    .push(ParseError::at(location, error.to_string()));
                None
            }
        }
    }

    fn parse_dimension(&mut self, value: &Value, location: &str) -> Option<Dimension> {
        let Some(object) = value.as_object() else {
            self.expected(location, "a dimension object");
            return None;
        };
        if !self.validate_object_keys(object, location, &["value", "unit"]) {
            return None;
        }

        let value = self.parse_number(
            object.get("value").unwrap_or(&Value::Null),
            &format!("{location}.value"),
        )?;
        let unit = match object.get("unit").and_then(Value::as_str) {
            Some("px") => DimensionUnit::Pixel,
            Some("rem") => DimensionUnit::Rem,
            Some(unit) => {
                self.errors.push(ParseError::at(
                    format!("{location}.unit"),
                    format!("unsupported dimension unit '{unit}'"),
                ));
                return None;
            }
            None => {
                self.expected(&format!("{location}.unit"), "'px' or 'rem'");
                return None;
            }
        };

        Dimension::new(value.get(), unit).ok()
    }

    fn validate_reserved_properties(
        &mut self,
        object: &Map<String, Value>,
        location: &str,
        token: bool,
    ) {
        for name in object.keys().filter(|name| name.starts_with('$')) {
            let allowed = matches!(
                name.as_str(),
                "$type" | "$description" | "$extensions" | "$deprecated"
            ) || token && name == "$value";
            if !allowed {
                self.errors.push(ParseError::at(
                    format!("{location}.{name}"),
                    format!("unsupported reserved property '{name}'"),
                ));
            }
        }
    }

    fn validate_object_keys(
        &mut self,
        object: &Map<String, Value>,
        location: &str,
        expected: &[&str],
    ) -> bool {
        let mut valid = true;
        for key in object.keys() {
            if !expected.contains(&key.as_str()) {
                self.errors.push(ParseError::at(
                    format!("{location}.{key}"),
                    format!("unexpected property '{key}'"),
                ));
                valid = false;
            }
        }
        for key in expected {
            if !object.contains_key(*key) {
                self.errors.push(ParseError::at(
                    format!("{location}.{key}"),
                    format!("missing property '{key}'"),
                ));
                valid = false;
            }
        }
        valid
    }

    fn expected(&mut self, location: &str, expected: &str) {
        self.errors
            .push(ParseError::at(location, format!("expected {expected}")));
    }
}

fn alias_path(value: &str) -> Option<&str> {
    value.strip_prefix('{')?.strip_suffix('}')
}

fn valid_name(name: &str) -> bool {
    !name.is_empty() && !name.contains(['.', '{', '}'])
}

fn json_path(path: &[String]) -> String {
    if path.is_empty() {
        "$".to_owned()
    } else {
        format!("$.{}", path.join("."))
    }
}

#[cfg(test)]
mod tests {
    use themosis_core::{DimensionUnit, SourceId, TokenExpression, TokenKind, TokenValue};

    use super::*;

    const VALID: &str = include_str!("../tests/fixtures/valid/theme.tokens.json");

    #[test]
    fn parses_supported_tokens_and_group_types() {
        let document = parse(SourceId::new(3), VALID).expect("fixture is valid");
        let paths: Vec<String> = document
            .tokens()
            .iter()
            .map(|token| token.path().to_string())
            .collect();

        assert_eq!(document.source(), SourceId::new(3));
        assert_eq!(
            paths,
            [
                "color.accent",
                "color.primary",
                "enabled",
                "label",
                "opacity",
                "spacing.small",
            ]
        );
        assert_eq!(document.tokens()[0].kind(), TokenKind::Color);
        assert!(matches!(
            document.tokens()[0].expression(),
            TokenExpression::Alias(path) if path.to_string() == "color.primary"
        ));
        assert!(matches!(
            document.tokens()[5].expression(),
            TokenExpression::Literal(TokenValue::Dimension(value))
                if value.unit() == DimensionUnit::Pixel && value.value().get() == 4.0
        ));
    }

    #[test]
    fn reports_all_structural_errors() {
        let input = include_str!("../tests/fixtures/invalid/structure.tokens.json");
        let errors = parse(SourceId::new(0), input).expect_err("fixture is invalid");
        let messages: Vec<&str> = errors
            .errors()
            .iter()
            .map(super::ParseError::message)
            .collect();

        assert!(messages.contains(&"token has no $type and does not inherit one"));
        assert!(messages.contains(&"unsupported token type 'gradient'"));
        assert!(messages.contains(&"unsupported reserved property '$unknown'"));
    }

    #[test]
    fn rejects_out_of_range_color_components() {
        let input = include_str!("../tests/fixtures/invalid/color.tokens.json");
        let errors = parse(SourceId::new(0), input).expect_err("fixture is invalid");

        assert!(errors.to_string().contains("color component red"));
    }

    #[test]
    fn reports_json_syntax_location() {
        let errors = parse(SourceId::new(0), "{\n  nope\n}").expect_err("JSON is invalid");
        let error = &errors.errors()[0];

        assert_eq!(error.line(), Some(2));
        assert!(error.column().is_some());
    }
}
