//! Shared native theme generation used by the addon builder, importer, dock,
//! and headless runners.
//!
//! Compilation goes through the same semantic pipeline as the CLI, so every
//! entry point reports identical dependencies and structured diagnostics.

use std::{collections::BTreeSet, path::PathBuf};

use godot::{
    classes::{ResourceLoader, Theme},
    obj::Singleton,
    prelude::*,
};
use themosis::compile_theme_with_report;
use themosis_core::{CompiledTheme, CompiledValue};
use themosis_godot::RunnerDiagnostic;

use crate::{
    native::{
        backend::build_theme,
        diagnostics::{build_diagnostics, load_diagnostics},
        import_cache::resolved_uid_path,
    },
    project::provider::GodotSourceProvider,
};

/// One compilation attempt: its dependency graph and its result.
#[derive(Debug)]
pub(crate) struct GenerationAttempt {
    /// Every source and `res://` resource the compilation read.
    pub(crate) dependencies: BTreeSet<String>,
    /// The compiled theme, or the structured failure that replaced it.
    pub(crate) result: Result<Gd<Theme>, GenerationFailure>,
}

/// Why a compilation attempt failed.
#[derive(Debug)]
pub(crate) struct GenerationFailure {
    /// Human-readable failure summary.
    pub(crate) message: String,
    /// Structured diagnostics with source locations.
    pub(crate) diagnostics: Vec<RunnerDiagnostic>,
}

impl GenerationAttempt {
    /// Returns the dependency graph as a list.
    pub(crate) fn dependency_list(&self) -> Vec<String> {
        self.dependencies.iter().cloned().collect()
    }
}

/// Compiles a `res://` root into a native theme or a structured failure.
pub(crate) fn generate_from_project_path(root_source: &str) -> GenerationAttempt {
    let source = root_source;
    let Some(relative) = source
        .strip_prefix("res://")
        .filter(|relative| !relative.is_empty())
    else {
        let message = format!("theme source '{source}' must be below res://");
        return GenerationAttempt {
            dependencies: BTreeSet::new(),
            result: Err(GenerationFailure {
                diagnostics: vec![RunnerDiagnostic::new("invalid_source", &message)],
                message,
            }),
        };
    };
    let report = compile_theme_with_report(&GodotSourceProvider::new(), PathBuf::from(relative));
    let mut dependencies = report
        .dependencies()
        .iter()
        .map(|path| format!("res://{}", path.display()))
        .collect::<BTreeSet<_>>();
    let compiled = match report.into_result() {
        Ok(compiled) => compiled,
        Err(error) => {
            return GenerationAttempt {
                dependencies,
                result: Err(GenerationFailure {
                    diagnostics: load_diagnostics(&error),
                    message: error.to_string(),
                }),
            };
        }
    };
    collect_resource_dependencies(&compiled, &mut dependencies);
    let result = build_theme(&compiled).map_err(|error| GenerationFailure {
        diagnostics: build_diagnostics(&error),
        message: error.to_string(),
    });
    GenerationAttempt {
        dependencies,
        result,
    }
}

/// Expands the compiled theme's resource references into the full resource
/// graph, using Godot's documented dependency API.
///
/// A nested resource — and the UID a reference resolves to — must invalidate
/// an import exactly like a direct dependency. Entries reported as
/// `uid://<...>` keep their identity in the manifest so re-pointing a UID is
/// visible to fingerprinting, and a reported fallback path is recorded as an
/// ordinary dependency. Traversal is deterministic and cycle-safe.
fn collect_resource_dependencies(compiled: &CompiledTheme, dependencies: &mut BTreeSet<String>) {
    let mut expanded = BTreeSet::new();
    let mut frontier = direct_resource_references(compiled);
    while let Some(reference) = frontier.pop_first() {
        if !expanded.insert(reference.clone()) {
            continue;
        }
        dependencies.insert(reference.clone());
        // An unresolvable UID still has its resolution fingerprinted; there is
        // nothing further to traverse.
        let Some(target) = traversal_target(&reference) else {
            continue;
        };
        for entry in resource_dependencies(&target) {
            for dependency in dependency_entries(&entry) {
                frontier.insert(dependency);
            }
        }
    }
}

/// Collects the theme's own `res://` resource references.
fn direct_resource_references(compiled: &CompiledTheme) -> BTreeSet<String> {
    let mut references = BTreeSet::new();
    for style in compiled.styles().values() {
        for value in style.properties().values().chain(
            style
                .states()
                .values()
                .flat_map(|state| state.properties().values()),
        ) {
            let CompiledValue::Resource(reference) = value else {
                continue;
            };
            if reference.as_str().starts_with("res://") {
                references.insert(reference.as_str().to_owned());
            }
        }
    }
    references
}

/// Returns the engine-reported dependency entries of one resource path.
fn resource_dependencies(path: &str) -> Vec<String> {
    ResourceLoader::singleton()
        .get_dependencies(path)
        .to_vec()
        .into_iter()
        .map(|entry| entry.to_string())
        .collect()
}

/// Splits one dependency entry into the strings that must be fingerprinted.
///
/// Godot reports a dependency either as a plain path or as a `uid::…::fallback`
/// triple whose second section is empty. Both the UID and its fallback path are
/// recorded, so re-pointing the UID and editing the fallback each invalidate an
/// import.
fn dependency_entries(entry: &str) -> Vec<String> {
    match entry.split("::").collect::<Vec<_>>().as_slice() {
        [path] if !path.is_empty() => vec![(*path).to_owned()],
        [uid, _, fallback] => [*uid, *fallback]
            .into_iter()
            .filter(|section| !section.is_empty())
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

/// Returns the path to traverse for a recorded dependency.
///
/// `uid://` references follow their current resolution when one exists;
/// everything else is traversed as recorded.
fn traversal_target(reference: &str) -> Option<String> {
    if reference.starts_with("uid://") {
        return resolved_uid_path(reference);
    }
    reference
        .starts_with("res://")
        .then(|| reference.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{dependency_entries, traversal_target};

    #[test]
    fn records_plain_and_uid_dependency_entries() {
        assert_eq!(
            dependency_entries("res://texture.tres"),
            ["res://texture.tres"]
        );
        assert_eq!(
            dependency_entries("uid://abc123::::res://fallback.tres"),
            ["uid://abc123", "res://fallback.tres"]
        );
        assert_eq!(
            dependency_entries("::::res://fallback.tres"),
            ["res://fallback.tres"]
        );
        assert!(dependency_entries("").is_empty());
    }

    #[test]
    fn traverses_recorded_resource_paths() {
        assert_eq!(
            traversal_target("res://texture.tres").as_deref(),
            Some("res://texture.tres")
        );
        assert_eq!(traversal_target("theme://font/body"), None);
    }
}
