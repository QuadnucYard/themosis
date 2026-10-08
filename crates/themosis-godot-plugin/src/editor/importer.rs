//! Native `EditorImportPlugin` turning `.tms` roots into imported `Theme`
//! resources.
//!
//! Godot owns the importer lifecycle; everything else is a Rust API. The
//! importer records one [`ImportReport`] per source, which the editor plugin
//! reads to update its dependency tracking and the dock. Artifact metadata and
//! fingerprinting live in [`crate::native::import_cache`].

// `#[class(init)]` expands to code that trips these lints on the `base` field.
#![allow(clippy::absolute_paths, clippy::redundant_field_names)]

use std::collections::BTreeMap;

use godot::{
    classes::{EditorImportPlugin, IEditorImportPlugin},
    global::Error,
    prelude::*,
};
use themosis_godot::runner::RunnerDiagnostic;

use crate::native::{generation::generate_from_project_path, import_cache::save_imported};

/// Registered importer name.
pub(crate) const IMPORTER_NAME: &str = "themosis.theme";
/// Import format version.
///
/// Version `3` records complete dependency manifests: the transitive resource
/// graph including UID references. Godot reimports every existing artifact
/// when the version changes, so older manifests refresh themselves.
pub(crate) const IMPORTER_VERSION: i32 = 3;

/// Result of one engine-driven import, kept until the plugin reads it.
#[derive(Clone, Debug)]
pub(crate) struct ImportReport {
    /// `res://` theme root the import compiled.
    pub(crate) source: String,
    /// Whether compilation and saving both finished.
    pub(crate) ok: bool,
    /// Dependency graph of the compiled theme.
    pub(crate) dependencies: Vec<String>,
    /// Failure message; empty on success.
    pub(crate) error: String,
    /// Structured failures; empty on success.
    pub(crate) diagnostics: Vec<RunnerDiagnostic>,
}

/// Godot-facing importer for `.tms` theme roots.
#[derive(GodotClass)]
#[class(tool, init, base=EditorImportPlugin)]
pub struct ThemosisThemeImporter {
    base: Base<EditorImportPlugin>,
    reports: BTreeMap<String, ImportReport>,
}

#[godot_api]
impl ThemosisThemeImporter {
    /// Emitted once per import after compilation and saving both finished.
    #[signal]
    pub fn import_completed(source: GString);
}

impl ThemosisThemeImporter {
    /// Takes the report of the most recent import of `source`, if any.
    pub(crate) fn take_report(&mut self, source: &str) -> Option<ImportReport> {
        self.reports.remove(source)
    }
}

#[godot_api]
impl IEditorImportPlugin for ThemosisThemeImporter {
    fn get_importer_name(&self) -> GString {
        GString::from(IMPORTER_NAME)
    }

    fn get_visible_name(&self) -> GString {
        GString::from("Themosis Theme")
    }

    fn get_format_version(&self) -> i32 {
        IMPORTER_VERSION
    }

    fn get_recognized_extensions(&self) -> PackedStringArray {
        let mut extensions = PackedStringArray::new();
        extensions.push("tms");
        extensions
    }

    fn get_save_extension(&self) -> GString {
        GString::from("tres")
    }

    fn get_resource_type(&self) -> GString {
        GString::from("Theme")
    }

    fn get_preset_count(&self) -> i32 {
        1
    }

    fn get_preset_name(&self, _preset_index: i32) -> GString {
        GString::from("Default")
    }

    fn get_import_options(&self, _path: GString, _preset_index: i32) -> Array<AnyDictionary> {
        Array::new()
    }

    fn can_import_threaded(&self) -> bool {
        // Compilation enters the running engine to inspect native theme metadata.
        false
    }

    fn import(
        &mut self,
        source_file: GString,
        save_path: GString,
        _options: VarDictionary,
        _platform_variants: Array<GString>,
        _gen_files: Array<GString>,
    ) -> Error {
        let source = source_file.to_string();
        let attempt = generate_from_project_path(&source);
        let dependencies = attempt.dependency_list();
        let (error, report) = match attempt.result {
            Err(failure) => {
                for diagnostic in &failure.diagnostics {
                    godot_error!(
                        "Themosis import [{}]: {}",
                        diagnostic.code,
                        diagnostic.message
                    );
                }
                (
                    Error::ERR_PARSE_ERROR,
                    ImportReport {
                        source: source.clone(),
                        ok: false,
                        dependencies,
                        error: failure.message,
                        diagnostics: failure.diagnostics,
                    },
                )
            }
            Ok(mut theme) => {
                match save_imported(&mut theme, &dependencies, &source, &save_path.to_string()) {
                    Ok(()) => (
                        Error::OK,
                        ImportReport {
                            source: source.clone(),
                            ok: true,
                            dependencies,
                            error: String::new(),
                            diagnostics: Vec::new(),
                        },
                    ),
                    Err(failure) => {
                        godot_error!("{}", failure.message);
                        (
                            failure.error,
                            ImportReport {
                                source: source.clone(),
                                ok: false,
                                dependencies,
                                error: failure.message,
                                diagnostics: failure.diagnostics,
                            },
                        )
                    }
                }
            }
        };
        // Report exactly one completion per import, after compilation and
        // saving have both finished, so consumers never observe a pre-save
        // success.
        self.reports.insert(source, report);
        self.signals().import_completed().emit(&source_file);
        error
    }
}
