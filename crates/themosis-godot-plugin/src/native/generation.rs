//! Shared native theme generation used by the addon builder, importer, dock,
//! and headless runners.
//!
//! Compilation goes through the same semantic pipeline as the CLI, so every
//! entry point reports identical dependencies and structured diagnostics.

use std::{collections::BTreeSet, path::PathBuf};

use godot::{classes::Theme, prelude::*};
use themosis::compile_theme_with_report;
use themosis_core::{CompiledTheme, CompiledValue};
use themosis_godot::RunnerDiagnostic;

use crate::{
    native::{
        backend::build_theme,
        diagnostics::{build_diagnostics, load_diagnostics},
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

fn collect_resource_dependencies(compiled: &CompiledTheme, dependencies: &mut BTreeSet<String>) {
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
                dependencies.insert(reference.as_str().to_owned());
            }
        }
    }
}
