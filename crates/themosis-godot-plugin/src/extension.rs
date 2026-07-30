mod diagnostic;

use std::{collections::BTreeSet, path::PathBuf};

use godot::{classes::Theme, prelude::*};
use themosis::compile_theme_with_report;
use themosis_core::{CompiledTheme, CompiledValue};

use self::diagnostic::{
    EditorDiagnostic, GenerationAttempt, GenerationFailure, build_diagnostics, load_diagnostics,
};
use crate::{backend::build_theme, provider::GodotSourceProvider};

struct ThemosisExtension;

#[gdextension]
unsafe impl ExtensionLibrary for ThemosisExtension {}

/// Godot-facing source compiler used by projects and editor tooling.
#[derive(GodotClass)]
#[class(init, base=RefCounted)]
pub struct ThemosisThemeGenerator {
    last_error: GString,
    last_dependencies: PackedStringArray,
    last_diagnostics: VarArray,
}

#[godot_api]
impl ThemosisThemeGenerator {
    /// Compiles a `res://` theme root and returns a native theme, or `null` on failure.
    #[func]
    fn generate(&mut self, root_source: GString) -> Option<Gd<Theme>> {
        let attempt = generate_from_project_path(&root_source);
        self.record_attempt(&attempt);
        attempt.result.ok()
    }

    /// Compiles a root and returns structured data for editor integrations.
    #[func]
    fn generate_result(&mut self, root_source: GString) -> VarDictionary {
        let attempt = generate_from_project_path(&root_source);
        self.record_attempt(&attempt);
        let mut result = VarDictionary::new();
        result.set("dependencies", &self.last_dependencies.to_variant());
        result.set("diagnostics", &self.last_diagnostics.to_variant());
        result.set("error", &self.last_error.to_variant());
        match attempt.result {
            Ok(theme) => {
                result.set("ok", true);
                result.set("theme", &theme.to_variant());
            }
            Err(_) => {
                result.set("ok", false);
                result.set("theme", &Variant::nil());
            }
        }
        result
    }

    /// Returns the failure from the most recent `generate` call.
    #[func]
    fn get_last_error(&self) -> GString {
        self.last_error.clone()
    }

    /// Returns the source and Godot resource dependencies from the most recent attempt.
    #[func]
    fn get_last_dependencies(&self) -> PackedStringArray {
        self.last_dependencies.clone()
    }

    /// Returns structured diagnostics from the most recent generation attempt.
    #[func]
    fn get_last_diagnostics(&self) -> VarArray {
        self.last_diagnostics.clone()
    }
}

impl ThemosisThemeGenerator {
    fn record_attempt(&mut self, attempt: &GenerationAttempt) {
        self.last_dependencies = attempt.dependencies.iter().map(Into::into).collect();
        match &attempt.result {
            Ok(_) => {
                self.last_error = GString::new();
                self.last_diagnostics = VarArray::new();
            }
            Err(failure) => {
                self.last_error = GString::from(&failure.message);
                self.last_diagnostics = failure
                    .diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.to_dictionary().to_variant())
                    .collect();
            }
        }
    }
}

fn generate_from_project_path(root_source: &GString) -> GenerationAttempt {
    let source = root_source.to_string();
    let Some(relative) = source
        .strip_prefix("res://")
        .filter(|relative| !relative.is_empty())
    else {
        let message = format!("theme source '{source}' must be below res://");
        return GenerationAttempt {
            dependencies: BTreeSet::new(),
            result: Err(GenerationFailure {
                diagnostics: vec![EditorDiagnostic::new("invalid_source", &message)],
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
