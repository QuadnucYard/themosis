use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

use themosis_core::{
    CompiledState, CompiledStyle, CompiledTheme, CompiledValue, Name, PropertyAssignment,
    ResolvedTokens, StyleDefinition, StyleDocument, StyleValue,
};

use crate::{CompileError, CompileErrors};

/// Compiles component styles against an already resolved token registry.
pub fn compile_styles(
    documents: &[StyleDocument],
    tokens: ResolvedTokens,
) -> Result<CompiledTheme, CompileErrors> {
    let Some(first) = documents.first() else {
        return Err(CompileErrors(vec![CompileError::NoStyleDocuments]));
    };
    let theme_name = first.name().clone();
    let mut definitions = BTreeMap::new();
    let mut errors = Vec::new();

    for document in documents {
        if document.name() != &theme_name {
            errors.push(CompileError::ThemeNameMismatch {
                expected: theme_name.clone(),
                found: document.name().clone(),
            });
        }
        for style in document.styles() {
            match definitions.entry(style.name().clone()) {
                Entry::Vacant(entry) => {
                    entry.insert(style.clone());
                }
                Entry::Occupied(entry) => {
                    errors.push(CompileError::DuplicateStyle {
                        style: entry.key().clone(),
                    });
                }
            }
        }
    }

    let compiled = {
        let mut resolver = StyleResolver::new(definitions, &tokens);
        resolver.resolve_all();
        errors.append(&mut resolver.errors);
        resolver.into_compiled()
    };

    if errors.is_empty() {
        Ok(CompiledTheme::new(theme_name, tokens, compiled))
    } else {
        Err(CompileErrors(errors))
    }
}

#[derive(Clone)]
struct PartialStyle {
    target: Name,
    properties: BTreeMap<Name, CompiledValue>,
    state_overrides: BTreeMap<Name, BTreeMap<Name, CompiledValue>>,
}

struct StyleResolver<'a> {
    definitions: BTreeMap<Name, StyleDefinition>,
    tokens: &'a ResolvedTokens,
    resolved: BTreeMap<Name, PartialStyle>,
    failed: BTreeSet<Name>,
    visiting: Vec<Name>,
    errors: Vec<CompileError>,
}

impl<'a> StyleResolver<'a> {
    fn new(definitions: BTreeMap<Name, StyleDefinition>, tokens: &'a ResolvedTokens) -> Self {
        Self {
            definitions,
            tokens,
            resolved: BTreeMap::new(),
            failed: BTreeSet::new(),
            visiting: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn resolve_all(&mut self) {
        let names: Vec<Name> = self.definitions.keys().cloned().collect();
        for name in names {
            self.resolve(&name);
        }
    }

    fn resolve(&mut self, name: &Name) -> Option<PartialStyle> {
        if let Some(style) = self.resolved.get(name) {
            return Some(style.clone());
        }
        if self.failed.contains(name) {
            return None;
        }
        if let Some(index) = self.visiting.iter().position(|candidate| candidate == name) {
            let mut cycle = self.visiting[index..].to_vec();
            cycle.push(name.clone());
            for member in &cycle {
                self.failed.insert(member.clone());
            }
            self.errors.push(CompileError::InheritanceCycle { cycle });
            return None;
        }

        let definition = self
            .definitions
            .get(name)
            .expect("resolver is only entered for known style names")
            .clone();
        self.visiting.push(name.clone());

        let resolved = self.resolve_definition(&definition);
        let popped = self.visiting.pop();
        debug_assert_eq!(popped.as_ref(), Some(name));

        match resolved {
            Some(style) => {
                self.resolved.insert(name.clone(), style.clone());
                Some(style)
            }
            None => {
                self.failed.insert(name.clone());
                None
            }
        }
    }

    fn resolve_definition(&mut self, definition: &StyleDefinition) -> Option<PartialStyle> {
        let mut style = if let Some(parent_name) = definition.extends() {
            if !self.definitions.contains_key(parent_name) {
                self.errors.push(CompileError::MissingParent {
                    style: definition.name().clone(),
                    parent: parent_name.clone(),
                });
                return None;
            }
            let parent = self.resolve(parent_name)?;
            if parent.target != *definition.target() {
                self.errors.push(CompileError::TargetMismatch {
                    style: definition.name().clone(),
                    target: definition.target().clone(),
                    parent: parent_name.clone(),
                    parent_target: parent.target,
                });
                return None;
            }
            parent
        } else {
            PartialStyle {
                target: definition.target().clone(),
                properties: BTreeMap::new(),
                state_overrides: BTreeMap::new(),
            }
        };

        let properties = self.compile_properties(definition, None, definition.properties());
        self.merge_properties(
            definition,
            None,
            &mut style.properties,
            &BTreeMap::new(),
            properties,
        );

        let mut seen_states = BTreeSet::new();
        for state in definition.states() {
            if !seen_states.insert(state.name().clone()) {
                self.errors.push(CompileError::DuplicateState {
                    style: definition.name().clone(),
                    state: state.name().clone(),
                });
                continue;
            }
            let properties =
                self.compile_properties(definition, Some(state.name()), state.properties());
            let base = style.properties.clone();
            let overrides = style
                .state_overrides
                .entry(state.name().clone())
                .or_default();
            self.merge_properties(definition, Some(state.name()), overrides, &base, properties);
        }

        Some(style)
    }

    fn compile_properties(
        &mut self,
        style: &StyleDefinition,
        state: Option<&Name>,
        assignments: &[PropertyAssignment],
    ) -> BTreeMap<Name, CompiledValue> {
        let mut properties = BTreeMap::new();
        for assignment in assignments {
            let value = match assignment.value() {
                StyleValue::Boolean(value) => Some(CompiledValue::Boolean(*value)),
                StyleValue::Number(value) => Some(CompiledValue::Number(*value)),
                StyleValue::String(value) => Some(CompiledValue::String(value.clone())),
                StyleValue::Resource(value) => Some(CompiledValue::Resource(value.clone())),
                StyleValue::Token(path) => match self.tokens.get(path) {
                    Some(value) => Some(CompiledValue::from(value.clone())),
                    None => {
                        self.errors.push(CompileError::MissingToken {
                            style: style.name().clone(),
                            state: state.cloned(),
                            property: assignment.name().clone(),
                            token: path.clone(),
                        });
                        None
                    }
                },
            };
            let Some(value) = value else {
                continue;
            };

            match properties.entry(assignment.name().clone()) {
                Entry::Vacant(entry) => {
                    entry.insert(value);
                }
                Entry::Occupied(entry) => {
                    self.errors.push(CompileError::DuplicateProperty {
                        style: style.name().clone(),
                        state: state.cloned(),
                        property: entry.key().clone(),
                    });
                }
            }
        }
        properties
    }

    fn merge_properties(
        &mut self,
        style: &StyleDefinition,
        state: Option<&Name>,
        destination: &mut BTreeMap<Name, CompiledValue>,
        fallback: &BTreeMap<Name, CompiledValue>,
        incoming: BTreeMap<Name, CompiledValue>,
    ) {
        for (name, value) in incoming {
            let expected = destination.get(&name).or_else(|| fallback.get(&name));
            if let Some(expected) = expected {
                if expected.kind() != value.kind() {
                    self.errors.push(CompileError::PropertyTypeMismatch {
                        style: style.name().clone(),
                        state: state.cloned(),
                        property: name,
                        expected: expected.kind(),
                        actual: value.kind(),
                    });
                    continue;
                }
            }
            destination.insert(name, value);
        }
    }

    fn into_compiled(self) -> BTreeMap<Name, CompiledStyle> {
        self.resolved
            .into_iter()
            .map(|(name, style)| {
                let states = style
                    .state_overrides
                    .into_iter()
                    .map(|(state_name, overrides)| {
                        let mut properties = style.properties.clone();
                        properties.extend(overrides);
                        (
                            state_name.clone(),
                            CompiledState::new(state_name, properties),
                        )
                    })
                    .collect();
                let compiled =
                    CompiledStyle::new(name.clone(), style.target, style.properties, states);
                (name, compiled)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use themosis_core::{
        CompiledValue, Name, Number, PropertyAssignment, ResolvedTokens, SourceId, StyleDefinition,
        StyleDocument, StyleState, StyleValue, TokenPath, TokenValue,
    };

    use super::{CompileError, compile_styles};

    fn path(value: &str) -> TokenPath {
        TokenPath::from_str(value).expect("test path is valid")
    }

    fn name(value: &str) -> Name {
        Name::new(value).expect("test name is valid")
    }

    fn property(name_value: &str, value: StyleValue) -> PropertyAssignment {
        PropertyAssignment::new(name(name_value), value)
    }

    fn style(
        style_name: &str,
        target: &str,
        parent: Option<&str>,
        properties: Vec<PropertyAssignment>,
        states: Vec<StyleState>,
    ) -> StyleDefinition {
        StyleDefinition::new(
            name(style_name),
            name(target),
            parent.map(name),
            properties,
            states,
        )
    }

    fn style_document(source: u32, theme: &str, styles: Vec<StyleDefinition>) -> StyleDocument {
        StyleDocument::new(
            SourceId::new(source),
            name(theme),
            Vec::new(),
            Vec::new(),
            styles,
        )
    }

    fn resolved_tokens() -> ResolvedTokens {
        ResolvedTokens::new([
            (
                path("color.primary"),
                TokenValue::String("primary".to_owned()),
            ),
            (
                path("color.accent"),
                TokenValue::String("accent".to_owned()),
            ),
        ])
    }

    #[test]
    fn compiles_tokens_inheritance_and_fully_expanded_states() {
        let base = style(
            "BaseButton",
            "Button",
            None,
            vec![
                property("background", StyleValue::Token(path("color.primary"))),
                property(
                    "font-size",
                    StyleValue::Number(Number::new(16.0).expect("number is finite")),
                ),
            ],
            vec![StyleState::new(
                name("hover"),
                vec![property(
                    "background",
                    StyleValue::Token(path("color.accent")),
                )],
            )],
        );
        let primary = style(
            "PrimaryButton",
            "Button",
            Some("BaseButton"),
            vec![property(
                "font-size",
                StyleValue::Number(Number::new(18.0).expect("number is finite")),
            )],
            Vec::new(),
        );

        let theme = compile_styles(
            &[style_document(0, "Application", vec![base, primary])],
            resolved_tokens(),
        )
        .expect("styles are valid");

        let primary = theme
            .styles()
            .get(&name("PrimaryButton"))
            .expect("child style was compiled");
        assert_eq!(
            primary.properties().get(&name("background")),
            Some(&CompiledValue::String("primary".to_owned()))
        );
        assert_eq!(
            primary.properties().get(&name("font-size")),
            Some(&CompiledValue::Number(
                Number::new(18.0).expect("number is finite")
            ))
        );
        let hover = primary
            .states()
            .get(&name("hover"))
            .expect("inherited state was compiled");
        assert_eq!(hover.properties().len(), 2);
        assert_eq!(
            hover.properties().get(&name("background")),
            Some(&CompiledValue::String("accent".to_owned()))
        );
        assert_eq!(
            hover.properties().get(&name("font-size")),
            Some(&CompiledValue::Number(
                Number::new(18.0).expect("number is finite")
            ))
        );
    }

    #[test]
    fn reports_missing_tokens_and_duplicate_declarations() {
        let duplicated = style(
            "ButtonStyle",
            "Button",
            None,
            vec![
                property("background", StyleValue::Token(path("color.missing"))),
                property("font-size", StyleValue::String("small".to_owned())),
                property("font-size", StyleValue::String("large".to_owned())),
            ],
            vec![
                StyleState::new(name("hover"), Vec::new()),
                StyleState::new(name("hover"), Vec::new()),
            ],
        );
        let document = style_document(0, "Application", vec![duplicated.clone(), duplicated]);

        let errors =
            compile_styles(&[document], resolved_tokens()).expect_err("styles are invalid");

        assert!(errors.errors().iter().any(|error| matches!(
            error,
            CompileError::DuplicateStyle { style } if style == &name("ButtonStyle")
        )));
        assert!(errors.errors().iter().any(|error| matches!(
            error,
            CompileError::MissingToken { token, .. } if token == &path("color.missing")
        )));
        assert!(errors.errors().iter().any(|error| matches!(
            error,
            CompileError::DuplicateProperty { property, .. } if property == &name("font-size")
        )));
        assert!(errors.errors().iter().any(|error| matches!(
            error,
            CompileError::DuplicateState { state, .. } if state == &name("hover")
        )));
    }

    #[test]
    fn reports_missing_parents_and_inheritance_cycles() {
        let orphan = style("Orphan", "Button", Some("Missing"), Vec::new(), Vec::new());
        let first = style("First", "Button", Some("Second"), Vec::new(), Vec::new());
        let second = style("Second", "Button", Some("First"), Vec::new(), Vec::new());

        let errors = compile_styles(
            &[style_document(
                0,
                "Application",
                vec![orphan, first, second],
            )],
            resolved_tokens(),
        )
        .expect_err("inheritance is invalid");

        assert!(errors.errors().iter().any(|error| matches!(
            error,
            CompileError::MissingParent { style, parent }
                if style == &name("Orphan") && parent == &name("Missing")
        )));
        assert!(errors.errors().iter().any(|error| matches!(
            error,
            CompileError::InheritanceCycle { cycle }
                if cycle == &vec![name("First"), name("Second"), name("First")]
        )));
    }

    #[test]
    fn rejects_target_and_property_kind_changes() {
        let base = style(
            "Base",
            "Button",
            None,
            vec![property(
                "font-size",
                StyleValue::Number(Number::new(16.0).expect("number is finite")),
            )],
            Vec::new(),
        );
        let wrong_target = style("LabelChild", "Label", Some("Base"), Vec::new(), Vec::new());
        let wrong_kind = style(
            "StringChild",
            "Button",
            Some("Base"),
            vec![property(
                "font-size",
                StyleValue::String("large".to_owned()),
            )],
            Vec::new(),
        );

        let errors = compile_styles(
            &[style_document(
                0,
                "Application",
                vec![base, wrong_target, wrong_kind],
            )],
            resolved_tokens(),
        )
        .expect_err("overrides are invalid");

        assert!(errors.errors().iter().any(|error| matches!(
            error,
            CompileError::TargetMismatch { style, .. } if style == &name("LabelChild")
        )));
        assert!(errors.errors().iter().any(|error| matches!(
            error,
            CompileError::PropertyTypeMismatch { style, property, .. }
                if style == &name("StringChild") && property == &name("font-size")
        )));
    }
}
