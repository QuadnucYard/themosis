//! Native editor plugin: registration, source indexing, dependency tracking,
//! and reimports.
//!
//! This plugin auto-registers while the editor loads the extension and cannot
//! be enabled or disabled through the plugins dialog. It talks to the importer
//! through Rust types; only the engine's own signals carry values between them.

// `#[class(init)]` expands to code that trips these lints on the `base` field.
#![allow(clippy::absolute_paths, clippy::redundant_field_names)]

use std::collections::{BTreeMap, BTreeSet};

use godot::{
    classes::{
        Button, EditorFileSystem, EditorPlugin, IEditorPlugin, Timer,
        editor_plugin::CustomControlContainer,
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
    editor::importer::{ImportReport, ThemosisThemeImporter},
    native::import_cache::{
        DEPENDENCIES_META, DEPENDENCY_FINGERPRINT_META, content_hash, fingerprint_snapshot,
        load_imported_theme, meta_string, meta_string_array,
    },
    project::sources,
};

/// Godot-facing editor plugin coordinating the importer and the reimport
/// toolbar.
#[derive(GodotClass)]
#[class(tool, init, base=EditorPlugin)]
pub struct ThemosisEditorPlugin {
    base: Base<EditorPlugin>,

    importer: Option<Gd<ThemosisThemeImporter>>,
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
        let importer = ThemosisThemeImporter::new_gd();
        importer
            .signals()
            .import_completed()
            .connect_other(&self.to_gd(), Self::on_import_completed);
        self.base_mut().add_import_plugin(&importer);

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
        let filesystem_handle = filesystem.clone().map(|filesystem| {
            filesystem
                .signals()
                .filesystem_changed()
                .connect_other(&self.to_gd(), Self::on_filesystem_changed)
        });

        self.importer = Some(importer);
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
            filesystem.scan();
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
        let sources = self.sources.clone();
        for source in sources {
            let dependencies = self
                .dependencies
                .get(&source)
                .cloned()
                .unwrap_or_else(|| vec![source.clone()]);
            let current = snapshot(&dependencies, &mut digests);
            let previous = self.snapshots.get(&source).cloned().unwrap_or_default();
            if current == previous {
                continue;
            }
            self.snapshots.insert(source.clone(), current);
            self.pending_reimports.insert(source.clone());
        }
        if !self.pending_reimports.is_empty()
            && let Some(mut refresh_timer) = self.refresh_timer.clone()
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
            if stored_fingerprint != fingerprint_snapshot(&current) {
                self.pending_reimports.insert(source.clone());
            }
        }
    }

    /// Records an outcome in dependency tracking.
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
    }

    /// Reacts to one finished engine import.
    fn on_import_completed(&mut self, source: GString) {
        let source = source.to_string();
        let report = self
            .importer
            .clone()
            .and_then(|mut importer| importer.bind_mut().take_report(&source));
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

    fn reimport_all(&mut self) {
        self.refresh_sources();
        for source in self.sources.clone() {
            self.pending_reimports.insert(source);
        }
        self.reimport_pending();
    }

    fn reimport_pending(&mut self) {
        if self.reimporting || self.pending_reimports.is_empty() {
            return;
        }
        let Some(filesystem) = self.filesystem.clone() else {
            return;
        };
        for source in &self.pending_reimports {
            if filesystem.clone().get_file_type(source.as_str()).is_empty() {
                // Startup discovery has not registered this source yet. Keep
                // the request queued until the filesystem scan completes.
                if let Some(mut refresh_timer) = self.refresh_timer.clone() {
                    refresh_timer.start();
                }
                return;
            }
        }
        let mut sources = Vec::new();
        for source in self.sources.clone() {
            if self.pending_reimports.contains(&source) {
                sources.push(source);
            }
        }
        self.pending_reimports.clear();
        if sources.is_empty() {
            return;
        }
        self.reimported.clear();
        self.reimporting = true;
        if let Some(mut button) = self.button.clone() {
            button.set_disabled(true);
            button.set_text("Importing Themosis…");
        }
        let mut packed = PackedStringArray::new();
        for source in &sources {
            packed.push(source.as_str());
        }
        filesystem.clone().reimport_files(&packed);
        self.reimported.clear();
        if let Some(mut button) = self.button.clone() {
            button.set_disabled(false);
            button.set_text("Reimport Themosis");
        }
        self.reimporting = false;
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
