//! Native editor plugin: registration, source indexing, dependency tracking,
//! and reimports.
//!
//! This plugin auto-registers while the editor loads the extension and cannot
//! be enabled or disabled through the plugins dialog. It talks to the importer
//! and the dock through Rust types; only the engine's own signals carry values
//! between them.

// `#[class(init)]` expands to code that trips these lints on the `base` field.
#![allow(clippy::absolute_paths, clippy::redundant_field_names)]

use std::collections::{BTreeMap, BTreeSet};

use godot::{
    classes::{
        Button, EditorFileSystem, EditorPlugin, IEditorPlugin, Timer,
        editor_plugin::{CustomControlContainer, DockSlot},
    },
    obj::{NewAlloc, NewGd, WithBaseField},
    prelude::*,
    signal::ConnectHandle,
};
use themosis_godot::{
    reports::{Operation, OperationOutcome, OutcomeStatus},
    runner::RunnerDiagnostic,
};

use crate::{
    editor::{
        dock::ThemosisThemeDock,
        importer::{ImportReport, ThemosisThemeImporter},
    },
    native::{
        builder,
        import_cache::{
            DEPENDENCIES_META, DEPENDENCY_FINGERPRINT_META, content_hash, fingerprint_snapshot,
            load_imported_theme, meta_string, meta_string_array,
        },
    },
    project::{config, sources},
};

/// Godot-facing editor plugin coordinating importer, dock, and toolbar.
#[derive(GodotClass)]
#[class(tool, init, base=EditorPlugin)]
pub struct ThemosisEditorPlugin {
    base: Base<EditorPlugin>,

    importer: Option<Gd<ThemosisThemeImporter>>,
    dock: Option<Gd<ThemosisThemeDock>>,
    button: Option<Gd<Button>>,
    filesystem: Option<Gd<EditorFileSystem>>,
    filesystem_handle: Option<ConnectHandle>,
    refresh_timer: Option<Gd<Timer>>,

    sources: Vec<String>,
    dependencies: BTreeMap<String, Vec<String>>,
    snapshots: BTreeMap<String, BTreeMap<String, String>>,
    pending_reimports: BTreeSet<String>,
    reimporting: bool,
    check_scheduled: bool,
    waiting_for_initial_scan: bool,
    reimported: BTreeSet<String>,
}

#[godot_api]
impl IEditorPlugin for ThemosisEditorPlugin {
    fn enter_tree(&mut self) {
        config::ensure_project_setting();

        let importer = ThemosisThemeImporter::new_gd();
        importer
            .signals()
            .import_completed()
            .connect_other(&self.to_gd(), Self::on_import_completed);
        self.base_mut().add_import_plugin(&importer);

        let dock = ThemosisThemeDock::new_alloc();
        dock.signals()
            .reimport_requested()
            .connect_other(&self.to_gd(), Self::reimport_one);
        dock.signals()
            .reimport_all_requested()
            .connect_other(&self.to_gd(), Self::reimport_all);
        dock.signals()
            .materialize_requested()
            .connect_other(&self.to_gd(), Self::materialize);
        dock.signals()
            .materialize_all_requested()
            .connect_other(&self.to_gd(), Self::materialize_all);
        dock.signals()
            .diagnostic_clicked()
            .connect_other(&self.to_gd(), Self::open_diagnostic);
        self.base_mut()
            .add_control_to_dock(DockSlot::RIGHT_UL, &dock);

        let mut button = Button::new_alloc();
        button.set_text("Reimport Themosis");
        button.set_tooltip_text("Reimport all Themosis .tms theme assets");
        button
            .signals()
            .pressed()
            .connect_other(&self.to_gd(), Self::reimport_all);
        self.base_mut()
            .add_control_to_container(CustomControlContainer::TOOLBAR, &button);

        let mut refresh_timer = Timer::new_alloc();
        refresh_timer.set_one_shot(true);
        refresh_timer.set_wait_time(0.35);
        refresh_timer
            .signals()
            .timeout()
            .connect_other(&self.to_gd(), Self::reimport_pending);
        self.base_mut().add_child(&refresh_timer);

        let filesystem = self
            .base()
            .get_editor_interface()
            .and_then(|editor| editor.get_resource_filesystem());
        let filesystem_handle = filesystem.as_ref().map(|filesystem| {
            filesystem
                .signals()
                .filesystem_changed()
                .connect_other(&self.to_gd(), Self::on_filesystem_changed)
        });

        self.importer = Some(importer);
        self.dock = Some(dock);
        self.button = Some(button);
        self.filesystem = filesystem;
        self.filesystem_handle = filesystem_handle;
        self.refresh_timer = Some(refresh_timer);
        self.base_mut().call_deferred("initialize_themes", &[]);
    }

    fn exit_tree(&mut self) {
        if let Some(handle) = self.filesystem_handle.take()
            && handle.is_connected()
        {
            handle.disconnect();
        }
        if let Some(mut dock) = self.dock.take() {
            self.base_mut().remove_control_from_docks(&dock);
            dock.queue_free();
        }
        if let Some(mut button) = self.button.take() {
            self.base_mut()
                .remove_control_from_container(CustomControlContainer::TOOLBAR, &button);
            button.queue_free();
        }
        if let Some(mut refresh_timer) = self.refresh_timer.take() {
            refresh_timer.queue_free();
        }
        if let Some(importer) = self.importer.take() {
            self.base_mut().remove_import_plugin(&importer);
        }
    }
}

#[godot_api]
impl ThemosisEditorPlugin {
    /// Discovers sources and starts the initial filesystem scan.
    #[func]
    fn initialize_themes(&mut self) {
        self.refresh_sources();
        // Registering an importer normally schedules discovery. An explicit
        // scan also handles .tms files that predate plugin activation.
        self.waiting_for_initial_scan = true;
        if let Some(mut filesystem) = self.filesystem.clone() {
            self.absorb_reentry(|| {
                filesystem.scan();
            });
        }
    }

    /// Reacts to filesystem changes by checking dependency fingerprints.
    #[func]
    fn check_dependency_changes(&mut self) {
        self.check_scheduled = false;
        let roots_changed = self.refresh_sources();
        if self.waiting_for_initial_scan {
            self.waiting_for_initial_scan = false;
            // Compare persisted dependency fingerprints only after Godot has
            // completed its ordinary source/importer scan. This avoids
            // rebuilding roots that Godot just imported because their .tms
            // file changed.
            self.index_all_sources();
            if !self.pending_reimports.is_empty() {
                self.reimport_pending();
            }
            return;
        }
        if roots_changed {
            for source in &self.sources {
                if !self.dependencies.contains_key(source) {
                    self.pending_reimports.insert(source.clone());
                }
            }
        }
        if self.reimporting {
            return;
        }
        let mut digests = BTreeMap::new();
        for source in &self.sources {
            let dependencies = self
                .dependencies
                .get(source)
                .cloned()
                .unwrap_or_else(|| vec![source.clone()]);
            let current = snapshot(&dependencies, &mut digests);
            let previous = self.snapshots.get(source).cloned().unwrap_or_default();
            if current == previous {
                continue;
            }
            self.snapshots.insert(source.clone(), current);
            self.pending_reimports.insert(source.clone());
            if let Some(dock) = self.dock.as_mut() {
                dock.bind_mut().mark_stale(source);
            }
        }
        if !self.pending_reimports.is_empty()
            && let Some(refresh_timer) = self.refresh_timer.as_mut()
        {
            refresh_timer.start();
        }
    }
}

impl ThemosisEditorPlugin {
    /// Returns whether the discovered source set changed.
    fn refresh_sources(&mut self) -> bool {
        let discovered = sources::discover_roots();
        if discovered == self.sources {
            return false;
        }
        self.sources = discovered;
        let sources = self.sources.clone();
        self.dependencies
            .retain(|source, _| sources.contains(source));
        self.snapshots.retain(|source, _| sources.contains(source));
        self.pending_reimports
            .retain(|source| sources.contains(source));
        if let Some(dock) = self.dock.as_mut() {
            dock.bind_mut().set_theme_sources(sources);
        }
        true
    }

    /// Indexes imported resources, reimporting roots whose fingerprints are
    /// missing or stale.
    fn index_all_sources(&mut self) {
        let mut digests = BTreeMap::new();
        for source in self.sources.clone() {
            let imported = load_imported_theme(&source);
            let mut dependencies = Vec::new();
            let mut stored_fingerprint = String::new();
            if let Some(imported) = &imported {
                dependencies = meta_string_array(imported, DEPENDENCIES_META);
                stored_fingerprint = meta_string(imported, DEPENDENCY_FINGERPRINT_META);
            }
            if dependencies.is_empty() {
                // Older or failed imports have no manifest. Let the importer
                // discover dependencies and report failures, rather than
                // compiling twice here.
                self.pending_reimports.insert(source);
                continue;
            }
            let current = snapshot(&dependencies, &mut digests);
            self.dependencies
                .insert(source.clone(), dependencies.clone());
            self.snapshots.insert(source.clone(), current.clone());
            self.show_outcome(OperationOutcome {
                operation: Operation::Import,
                profile: String::new(),
                source: source.clone(),
                output: String::new(),
                status: OutcomeStatus::Success,
                error: String::new(),
                diagnostics: Vec::new(),
                dependencies,
                elapsed_ms: 0,
            });
            if stored_fingerprint != fingerprint_snapshot(&current) {
                self.pending_reimports.insert(source.clone());
                if let Some(dock) = self.dock.as_mut() {
                    dock.bind_mut().mark_stale(&source);
                }
            }
        }
    }

    /// Records an outcome in dependency tracking and shows it in the dock.
    fn record_outcome(&mut self, outcome: OperationOutcome) {
        let dependencies = if outcome.dependencies.is_empty() {
            vec![outcome.source.clone()]
        } else {
            outcome.dependencies.clone()
        };
        self.dependencies
            .insert(outcome.source.clone(), dependencies.clone());
        self.snapshots.insert(
            outcome.source.clone(),
            snapshot(&dependencies, &mut BTreeMap::new()),
        );
        self.show_outcome(outcome);
    }

    /// Shows an outcome in the dock without touching dependency tracking.
    fn show_outcome(&mut self, outcome: OperationOutcome) {
        if let Some(dock) = self.dock.as_mut() {
            dock.bind_mut().apply_outcome(outcome);
        }
    }

    /// Reacts to one finished engine import.
    fn on_import_completed(&mut self, source: GString) {
        let source = source.to_string();
        let report = self
            .importer
            .as_mut()
            .and_then(|importer| importer.bind_mut().take_report(&source));
        self.reimported.insert(source.clone());
        let outcome = match report {
            Some(report) => report_outcome(report),
            None => unreported_outcome(&source),
        };
        self.record_outcome(outcome);
    }

    fn on_filesystem_changed(&mut self) {
        if self.check_scheduled {
            return;
        }
        self.check_scheduled = true;
        self.base_mut()
            .call_deferred("check_dependency_changes", &[]);
    }

    fn reimport_one(&mut self, source: GString) {
        if source.is_empty() {
            return;
        }
        self.pending_reimports.insert(source.to_string());
        self.reimport_pending();
    }

    fn reimport_all(&mut self) {
        self.refresh_sources();
        for source in &self.sources {
            self.pending_reimports.insert(source.clone());
        }
        self.reimport_pending();
    }

    fn reimport_pending(&mut self) {
        if self.reimporting || self.pending_reimports.is_empty() {
            return;
        }
        let Some(mut filesystem) = self.filesystem.clone() else {
            return;
        };
        for source in &self.pending_reimports {
            if filesystem.get_file_type(source.as_str()).is_empty() {
                // Startup discovery has not registered this source yet. Keep
                // the request queued until the filesystem scan completes.
                if let Some(refresh_timer) = self.refresh_timer.as_mut() {
                    refresh_timer.start();
                }
                return;
            }
        }
        let mut sources = Vec::new();
        for source in &self.sources {
            if self.pending_reimports.contains(source) {
                if let Some(dock) = self.dock.as_mut() {
                    dock.bind_mut().mark_importing(source);
                }
                sources.push(source.clone());
            }
        }
        self.pending_reimports.clear();
        if sources.is_empty() {
            return;
        }
        self.reimported.clear();
        self.reimporting = true;
        if let Some(button) = self.button.as_mut() {
            button.set_disabled(true);
            button.set_text("Importing Themosis…");
        }
        let mut packed = PackedStringArray::new();
        for source in &sources {
            packed.push(source.as_str());
        }
        self.absorb_reentry(|| {
            filesystem.reimport_files(&packed);
        });
        // A failed importer may not emit a usable resource notification on
        // every supported Godot version, so refresh the structured result
        // explicitly for the sources the importer did not report. Recompiling
        // a source whose import failed would otherwise replace its structured
        // save failure with a bare generation success. These fallback outcomes
        // go through dependency tracking so an unreported source is not
        // treated as perpetually stale.
        for source in sources {
            if self.reimported.contains(&source) {
                continue;
            }
            let outcome = self
                .importer
                .as_mut()
                .and_then(|importer| importer.bind_mut().take_report(&source))
                .map_or_else(|| unreported_outcome(&source), report_outcome);
            self.record_outcome(outcome);
        }
        self.reimported.clear();
        if let Some(button) = self.button.as_mut() {
            button.set_disabled(false);
            button.set_text("Reimport Themosis");
        }
        self.reimporting = false;
    }

    fn materialize(&mut self, source: GString, output: GString) {
        let output = output.to_string();
        let outcome = builder::materialize_source(&source.to_string(), &output);
        let succeeded = outcome.ok();
        self.show_outcome(outcome);
        if succeeded && let Some(mut filesystem) = self.filesystem.clone() {
            self.absorb_reentry(|| {
                filesystem.update_file(output.as_str());
            });
        }
    }

    fn materialize_all(&mut self, directory: GString) {
        let batch = builder::materialize_all(&self.sources, &directory.to_string());
        let updates = batch
            .results
            .iter()
            .filter(|result| result.ok() && !result.output.is_empty())
            .map(|result| result.output.clone())
            .collect::<Vec<_>>();
        if let Some(dock) = self.dock.as_mut() {
            for result in batch.results.iter().cloned() {
                dock.bind_mut().apply_outcome(result);
            }
        }
        if let Some(mut filesystem) = self.filesystem.clone() {
            self.absorb_reentry(|| {
                for output in &updates {
                    filesystem.update_file(output.as_str());
                }
            });
        }
        if let Some(dock) = self.dock.as_mut() {
            dock.bind_mut().show_batch(&batch);
        }
    }

    fn open_diagnostic(&mut self, path: GString) {
        if let Some(mut editor) = self.base().get_editor_interface() {
            editor.select_file(&path);
        }
    }

    /// Runs an engine call that can synchronously re-enter this plugin.
    ///
    /// `EditorFileSystem` emits `filesystem_changed`, and an import runs the
    /// importer, which emits `import_completed` — both while the engine call
    /// is still on the stack. The live `base_mut()` guard marks this plugin's
    /// Rust borrow as inaccessible for the duration, so those signal callbacks
    /// may borrow the plugin again instead of failing the borrow safeguard and
    /// losing the callback. The closure must only touch engine handles that do
    /// not borrow `self`.
    fn absorb_reentry(&mut self, engine_call: impl FnOnce()) {
        let _guard = self.base_mut();
        engine_call();
    }
}

/// Converts one importer report into an operation outcome.
fn report_outcome(report: ImportReport) -> OperationOutcome {
    OperationOutcome {
        operation: Operation::Import,
        profile: String::new(),
        source: report.source,
        output: String::new(),
        status: if report.ok {
            OutcomeStatus::Success
        } else {
            OutcomeStatus::Failure
        },
        error: report.error,
        diagnostics: report.diagnostics,
        dependencies: report.dependencies,
        elapsed_ms: 0,
    }
}

/// Builds the failure reported when an import leaves no usable result.
fn unreported_outcome(source: &str) -> OperationOutcome {
    let message = "import did not report a completion";
    OperationOutcome::failure(
        Operation::Import,
        "",
        source,
        "",
        message,
        vec![RunnerDiagnostic::new("import_unreported", message).at_source(source)],
    )
}

/// Hashes every dependency, reusing previously computed digests.
fn snapshot(paths: &[String], digests: &mut BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();
    for path in paths {
        let digest = digests
            .entry(path.clone())
            .or_insert_with(|| content_hash(path));
        result.insert(path.clone(), digest.clone());
    }
    result
}
