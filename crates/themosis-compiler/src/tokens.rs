use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

use themosis_core::{
    ResolvedTokens, TokenDefinition, TokenDocument, TokenExpression, TokenPath, TokenValue,
};

use crate::{CompileError, CompileErrors};

/// Merges and resolves token declarations from all supplied documents.
pub fn resolve_tokens(documents: &[TokenDocument]) -> Result<ResolvedTokens, CompileErrors> {
    let mut definitions = BTreeMap::new();
    let mut errors = Vec::new();

    for document in documents {
        for definition in document.tokens() {
            let path = definition.path().clone();
            match definitions.entry(path) {
                Entry::Vacant(entry) => {
                    entry.insert(definition.clone());
                }
                Entry::Occupied(entry) => {
                    errors.push(CompileError::DuplicateToken {
                        path: entry.key().clone(),
                    });
                }
            }
        }
    }

    let mut resolver = Resolver::new(definitions);
    resolver.resolve_all();
    errors.extend(resolver.errors);

    if errors.is_empty() {
        Ok(ResolvedTokens::new(resolver.resolved))
    } else {
        Err(CompileErrors(errors))
    }
}

struct Resolver {
    definitions: BTreeMap<TokenPath, TokenDefinition>,
    resolved: BTreeMap<TokenPath, TokenValue>,
    failed: BTreeSet<TokenPath>,
    visiting: Vec<TokenPath>,
    errors: Vec<CompileError>,
}

impl Resolver {
    fn new(definitions: BTreeMap<TokenPath, TokenDefinition>) -> Self {
        Self {
            definitions,
            resolved: BTreeMap::new(),
            failed: BTreeSet::new(),
            visiting: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn resolve_all(&mut self) {
        let paths: Vec<TokenPath> = self.definitions.keys().cloned().collect();
        for path in paths {
            self.resolve(&path);
        }
    }

    fn resolve(&mut self, path: &TokenPath) -> Option<TokenValue> {
        if let Some(value) = self.resolved.get(path) {
            return Some(value.clone());
        }
        if self.failed.contains(path) {
            return None;
        }
        if let Some(index) = self.visiting.iter().position(|candidate| candidate == path) {
            let mut cycle = self.visiting[index..].to_vec();
            cycle.push(path.clone());
            for member in &cycle {
                self.failed.insert(member.clone());
            }
            self.errors.push(CompileError::AliasCycle { cycle });
            return None;
        }

        let definition = self
            .definitions
            .get(path)
            .expect("resolver is only entered for known token paths")
            .clone();
        self.visiting.push(path.clone());

        let value = match definition.expression() {
            TokenExpression::Literal(value) => Some(value.clone()),
            TokenExpression::Alias(target) => self.resolve_alias(&definition, target),
        };

        let popped = self.visiting.pop();
        debug_assert_eq!(popped.as_ref(), Some(path));

        match value {
            Some(value) => {
                self.resolved.insert(path.clone(), value.clone());
                Some(value)
            }
            None => {
                self.failed.insert(path.clone());
                None
            }
        }
    }

    fn resolve_alias(
        &mut self,
        definition: &TokenDefinition,
        target: &TokenPath,
    ) -> Option<TokenValue> {
        if !self.definitions.contains_key(target) {
            self.errors.push(CompileError::MissingAlias {
                token: definition.path().clone(),
                target: target.clone(),
            });
            return None;
        }

        let value = self.resolve(target)?;
        if value.kind() != definition.kind() {
            self.errors.push(CompileError::TypeMismatch {
                token: definition.path().clone(),
                declared: definition.kind(),
                actual: value.kind(),
            });
            return None;
        }

        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use themosis_core::{
        Number, SourceId, TokenDefinition, TokenDocument, TokenExpression, TokenKind, TokenPath,
        TokenValue,
    };

    use super::{CompileError, resolve_tokens};

    fn path(value: &str) -> TokenPath {
        TokenPath::from_str(value).expect("test path is valid")
    }

    fn number(name: &str, value: f64) -> TokenDefinition {
        TokenDefinition::new(
            path(name),
            TokenKind::Number,
            TokenExpression::Literal(TokenValue::Number(
                Number::new(value).expect("test number is finite"),
            )),
        )
        .expect("literal matches declared type")
    }

    fn alias(name: &str, kind: TokenKind, target: &str) -> TokenDefinition {
        TokenDefinition::new(path(name), kind, TokenExpression::Alias(path(target)))
            .expect("aliases are unresolved")
    }

    #[test]
    fn resolves_forward_and_cross_document_aliases() {
        let documents = [
            TokenDocument::new(
                SourceId::new(0),
                vec![alias("spacing.control", TokenKind::Number, "spacing.base")],
            ),
            TokenDocument::new(SourceId::new(1), vec![number("spacing.base", 8.0)]),
        ];

        let resolved = resolve_tokens(&documents).expect("graph is valid");

        assert_eq!(resolved.len(), 2);
        assert_eq!(
            resolved.get(&path("spacing.control")),
            Some(&TokenValue::Number(
                Number::new(8.0).expect("number is finite")
            ))
        );
    }

    #[test]
    fn reports_duplicate_and_missing_tokens() {
        let documents = [
            TokenDocument::new(
                SourceId::new(0),
                vec![
                    number("spacing.base", 8.0),
                    alias("spacing.missing", TokenKind::Number, "unknown"),
                ],
            ),
            TokenDocument::new(SourceId::new(1), vec![number("spacing.base", 4.0)]),
        ];

        let errors = resolve_tokens(&documents).expect_err("graph is invalid");

        assert_eq!(errors.errors().len(), 2);
        assert!(matches!(
            &errors.errors()[0],
            CompileError::DuplicateToken { path: token } if token == &path("spacing.base")
        ));
        assert!(matches!(
            &errors.errors()[1],
            CompileError::MissingAlias { token, target }
                if token == &path("spacing.missing") && target == &path("unknown")
        ));
    }

    #[test]
    fn reports_a_closed_alias_cycle_once() {
        let documents = [TokenDocument::new(
            SourceId::new(0),
            vec![
                alias("a", TokenKind::Number, "b"),
                alias("b", TokenKind::Number, "c"),
                alias("c", TokenKind::Number, "a"),
            ],
        )];

        let errors = resolve_tokens(&documents).expect_err("graph has a cycle");

        assert_eq!(errors.errors().len(), 1);
        assert!(matches!(
            &errors.errors()[0],
            CompileError::AliasCycle { cycle }
                if cycle == &vec![path("a"), path("b"), path("c"), path("a")]
        ));
    }

    #[test]
    fn checks_alias_type_compatibility() {
        let documents = [TokenDocument::new(
            SourceId::new(0),
            vec![
                number("opacity.base", 0.8),
                alias("label.opacity", TokenKind::String, "opacity.base"),
            ],
        )];

        let errors = resolve_tokens(&documents).expect_err("alias type is wrong");

        assert!(matches!(
            &errors.errors()[0],
            CompileError::TypeMismatch {
                token,
                declared: TokenKind::String,
                actual: TokenKind::Number,
            } if token == &path("label.opacity")
        ));
    }
}
