//! Shared Godot infrastructure for the plugin's integration suites.
//!
//! `tests/godot` is the maintained probe project: a minimal Godot project used
//! to load the built GDExtension, plus the resources the native probes read.
//! Suites assemble throwaway copies of it, register the extension library, and
//! add local fixtures, so they never depend on the gallery in `examples/godot`.
//! The gallery is exercised only by the demo smoke tests in `tests/gallery.rs`,
//! which run against their own throwaway copy of it.
//!
//! The library a suite loads is pinned by `THEMOSIS_GODOT_LIBRARY`
//! (`scripts/godot-tests.nu` sets it after building the plugin with the
//! `test-support` feature) and verified to carry the probe classes. Without the
//! variable, the path is derived from this build's profile. A missing engine,
//! library, or probe class skips locally with a message;
//! `THEMOSIS_REQUIRE_GODOT` — set by the Godot CI job — turns those skips into
//! failures instead.
#![allow(dead_code)]

use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

use tempfile::{Builder as TempDirBuilder, TempDir};
use themosis_godot::{RunnerRequest, RunnerResponse};

/// The maintained probe project that loads the GDExtension.
const PROBE_PROJECT: &str = "tests/godot";

/// Probe project files every throwaway project starts from.
const PROBE_FILES: &[&str] = &[
    "project.godot",
    "main.tscn",
    "probe_font.tres",
    "probe_icon.tres",
    "probe_stylebox.tres",
];

/// Extension library the suites must load, pinned by `scripts/godot-tests.nu`.
const LIBRARY_ENV: &str = "THEMOSIS_GODOT_LIBRARY";

/// Turns missing prerequisites into failures; set by the Godot CI job.
const REQUIRE_ENV: &str = "THEMOSIS_REQUIRE_GODOT";

/// Class only registered by the `test-support` feature.
const PROBE_CLASS: &str = "ThemosisBackendTests";

/// Token fixture shared by the local themes.
const TOKENS: &str = r#"{
    "background": {
        "$type": "color",
        "$value": { "colorSpace": "srgb", "components": [0.1, 0.2, 0.3], "alpha": 1.0 }
    }
}"#;

/// Local root exercising colour and constant items.
const PROBE_THEME: &str = r#"theme Probe {
    tokens "tokens.json"
    style Card target=Panel {
        token panel background
    }
    style Gap target=VBoxContainer {
        number separation 12
    }
}"#;

/// First local root used by the profile workflows.
const ALPHA_THEME: &str = r#"theme Alpha {
    tokens "tokens.json"
    style Card target=Panel {
        token panel background
    }
}"#;

/// Second local root used by the profile workflows.
const BETA_THEME: &str = r#"theme Beta {
    tokens "tokens.json"
    style Gap target=VBoxContainer {
        number separation 12
    }
}"#;

/// Profile configuration materializing both local roots.
const PROFILE_CONFIG: &str = r#"{
  "active_profile": "alpha",
  "profiles": [
    {
      "auto_refresh": false,
      "build_on_start": false,
      "enabled": true,
      "name": "alpha",
      "output": "res://.themosis/materialized/alpha.tres",
      "preview": "none",
      "source": "res://theme/alpha.tms"
    },
    {
      "auto_refresh": false,
      "build_on_start": false,
      "enabled": true,
      "name": "beta",
      "output": "res://.themosis/materialized/beta.tres",
      "preview": "none",
      "source": "res://theme/beta.tms"
    }
  ],
  "version": 1
}
"#;

/// Reports a missing prerequisite, failing when the environment requires the
/// suite to run.
fn missing<T>(prerequisite: &str, hint: &str) -> Option<T> {
    assert!(
        !std::env::var_os(REQUIRE_ENV).is_some_and(|value| value != "0"),
        "required for this run, but missing: {prerequisite}; {hint}"
    );
    eprintln!("skipping Godot test: {prerequisite} is missing; {hint}");
    None
}

/// Returns the Godot executable, if one is installed.
pub fn godot() -> Option<String> {
    if let Some(executable) = std::env::var_os("THEMOSIS_GODOT_BINARY") {
        return Some(executable.to_string_lossy().into_owned());
    }
    for executable in ["godot", "godot4"] {
        if Command::new(executable).arg("--version").output().is_ok() {
            return Some(executable.to_owned());
        }
    }
    missing("Godot", "install Godot 4.5+ or set THEMOSIS_GODOT_BINARY")
}

/// Locates the GDExtension library the suites must load.
///
/// `THEMOSIS_GODOT_LIBRARY` pins the artifact built by `scripts/godot-tests.nu`
/// so a suite can never silently load an older, differently configured build;
/// without the variable the path is derived from this build's profile.
pub fn library() -> Option<PathBuf> {
    if let Some(pinned) = std::env::var_os(LIBRARY_ENV) {
        let pinned = PathBuf::from(pinned);
        assert!(
            pinned.is_file(),
            "THEMOSIS_GODOT_LIBRARY is not a file: {}",
            pinned.display()
        );
        return Some(pinned);
    }
    let profile = option_env!("PROFILE").unwrap_or("debug");
    let name = format!(
        "{}themosis_godot_plugin{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    );
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(profile)
        .join("deps")
        .join(name);
    if !path.is_file() {
        return missing(
            "the GDExtension library",
            "run `just test-godot` to build it with the `test-support` feature",
        );
    }
    Some(path)
}

/// Locates the library and verifies it carries the `test-support` probes.
///
/// A pinned library without the probes is a configuration error, because
/// `scripts/godot-tests.nu` builds exactly that feature set; a derived library
/// is a local skip with instructions.
pub fn probe_library() -> Option<PathBuf> {
    let path = library()?;
    let probes = std::fs::read(&path)
        .expect("extension library is readable")
        .windows(PROBE_CLASS.len())
        .any(|window| window == PROBE_CLASS.as_bytes());
    if probes {
        return Some(path);
    }
    assert!(
        std::env::var_os(LIBRARY_ENV).is_none(),
        "{} was built without the `test-support` feature; run `just test-godot`",
        path.display()
    );
    missing(
        "the `test-support` probe classes",
        "run `just test-godot` to rebuild the library",
    )
}

/// Merges a Godot process's streams for sentinel and error inspection.
pub fn messages(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// A throwaway Godot project that registers the built extension and probes.
pub struct ProbeProject {
    directory: TempDir,
    logs: TempDir,
}

impl ProbeProject {
    /// Creates a temporary copy of the probe project.
    ///
    /// The project keeps the probe settings, registers the built extension
    /// library in `themosis.gdextension` and `.godot/extension_list.cfg`, and
    /// binds the native addon classes from the library. Returns `None` when
    /// the extension library has not been built yet, like the runtime-backed
    /// CLI tests.
    pub fn new() -> Option<Self> {
        let library = library()?;
        let project = Self {
            directory: TempDirBuilder::new()
                .prefix("themosis-probe-test-")
                .tempdir()
                .expect("temporary Godot project is created"),
            logs: tempfile::tempdir().expect("Godot test log directory is created"),
        };
        for file in PROBE_FILES {
            std::fs::copy(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(PROBE_PROJECT)
                    .join(file),
                project.path().join(file),
            )
            .unwrap_or_else(|error| panic!("probe file '{file}' is copied: {error}"));
        }
        write_extension_manifest(project.path(), &library);
        std::fs::create_dir_all(project.path().join(".godot"))
            .expect("Godot metadata directory is created");
        std::fs::write(
            project.path().join(".godot/extension_list.cfg"),
            "res://themosis.gdextension\n",
        )
        .expect("extension registration is written");
        Some(project)
    }

    /// Returns the temporary project directory.
    pub fn path(&self) -> &Path {
        self.directory.path()
    }

    /// Returns a log path outside the project tree.
    pub fn log(&self, name: &str) -> PathBuf {
        self.logs.path().join(name)
    }

    /// Writes a project-relative file, creating its parent directories.
    pub fn write(&self, path: &str, contents: &str) {
        write_project_file(self.path(), path, contents);
    }

    /// Writes the local token and theme fixture.
    pub fn write_theme_fixture(&self) {
        self.write("theme/tokens.json", TOKENS);
        self.write("theme/probe.tms", PROBE_THEME);
    }

    /// Writes the two independent roots used by the profile workflows.
    pub fn write_profile_fixtures(&self) {
        self.write("theme/tokens.json", TOKENS);
        self.write("theme/alpha.tms", ALPHA_THEME);
        self.write("theme/beta.tms", BETA_THEME);
    }

    /// Writes the profile configuration materializing both fixture roots.
    pub fn write_profile_config(&self) {
        self.write("themosis.godot.json", PROFILE_CONFIG);
    }

    /// Runs a project script in a headless session.
    pub fn run_script(&self, executable: &str, script: &str, log_name: &str) -> Output {
        run_project_script(self.path(), &self.log(log_name), executable, script)
    }

    /// Runs the native CLI runner through Godot's `--main-loop`.
    pub fn run_main_loop(
        &self,
        executable: &str,
        request: &RunnerRequest,
        log_name: &str,
    ) -> (std::process::ExitStatus, RunnerResponse) {
        let files = tempfile::tempdir().expect("runner protocol directory is created");
        let request_path = files.path().join("request.json");
        let response_path = files.path().join("response.json");
        std::fs::write(&request_path, request.to_json()).expect("runner request is written");

        let output = Command::new(executable)
            .args(["--headless", "--path"])
            .arg(self.path())
            .arg("--log-file")
            .arg(self.log(log_name))
            .arg("--main-loop")
            .arg("ThemosisCliRunner")
            .arg("--")
            .arg(&request_path)
            .arg(&response_path)
            .output()
            .expect("Godot runner starts");

        let text = std::fs::read_to_string(&response_path).unwrap_or_else(|error| {
            panic!(
                "runner produced no response ({error}):\n{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
        });
        let response = RunnerResponse::from_json(&text)
            .unwrap_or_else(|error| panic!("runner response is malformed: {error}: {text}"));
        (output.status, response)
    }

    /// Runs the native build runner with user arguments.
    pub fn run_build_runner(&self, executable: &str, arguments: &[&str], log_name: &str) -> Output {
        Command::new(executable)
            .args(["--headless", "--path"])
            .arg(self.path())
            .arg("--log-file")
            .arg(self.log(log_name))
            .args(["--main-loop", "ThemosisBuildRunner", "--"])
            .args(arguments)
            .output()
            .expect("Godot build runner starts")
    }

    /// Runs the native import validation gate.
    pub fn run_check_runner(&self, executable: &str, log_name: &str) -> Output {
        Command::new(executable)
            .args(["--headless", "--path"])
            .arg(self.path())
            .arg("--log-file")
            .arg(self.log(log_name))
            .args(["--main-loop", "ThemosisCheckRunner"])
            .output()
            .expect("Godot import gate starts")
    }

    /// Imports the project in an editor session.
    pub fn run_import(&self, executable: &str, log_name: &str) -> Output {
        Command::new(executable)
            .args(["--headless", "--editor", "--path"])
            .arg(self.path())
            .arg("--log-file")
            .arg(self.log(log_name))
            .arg("--import")
            .output()
            .expect("Godot project import starts")
    }

    /// Returns the imported cache entry for the probe theme, when it exists.
    pub fn imported_theme_path(&self) -> Option<PathBuf> {
        let cache = self.path().join(".godot/imported");
        std::fs::read_dir(cache)
            .ok()?
            .map(|entry| entry.expect("cache entry").path())
            .find(|path| {
                path.file_name()
                    .expect("file")
                    .to_string_lossy()
                    .starts_with("probe.tms")
                    && path
                        .extension()
                        .is_some_and(|extension| extension == "tres")
            })
    }

    /// Returns an imported cache entry's text, or `None` when it is missing.
    pub fn imported_theme_text(&self) -> Option<String> {
        let entry = self.imported_theme_path()?;
        std::fs::read_to_string(entry).ok()
    }
}

/// Writes a `.gdextension` manifest that loads `library` on every platform key.
///
/// The pinned library is the only artifact a suite can load, so a run can never
/// validate a different build than the one it was configured with.
fn write_extension_manifest(project: &Path, library: &Path) {
    let library = library.to_str().expect("extension library path is UTF-8");
    let mut libraries = String::new();
    for key in [
        "macos.debug",
        "macos.debug.arm64",
        "macos.debug.x86_64",
        "linux.debug.x86_64",
        "linux.debug.arm64",
        "windows.debug.x86_64",
        "windows.debug.arm64",
    ] {
        libraries.push_str(&format!("{key} = \"{library}\"\n"));
    }
    std::fs::write(
        project.join("themosis.gdextension"),
        format!(
            "[configuration]\nentry_symbol = \"gdext_rust_init\"\ncompatibility_minimum = 4.5\nreloadable = true\n\n[libraries]\n{libraries}"
        ),
    )
    .expect("extension manifest is written");
}

/// Writes a project-relative file, creating its parent directories.
fn write_project_file(project: &Path, path: &str, contents: &str) {
    let path = project.join(path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("fixture directory is created");
    }
    std::fs::write(&path, contents).expect("fixture file is written");
}

/// Runs one project script in a headless session.
fn run_project_script(project: &Path, log: &Path, executable: &str, script: &str) -> Output {
    Command::new(executable)
        .args(["--headless", "--path"])
        .arg(project)
        .arg("--log-file")
        .arg(log)
        .arg("--script")
        .arg(script)
        .output()
        .expect("Godot script session starts")
}

/// Copies a project tree, skipping engine caches and the addon folder.
fn copy_project(source: &Path, destination: &Path) {
    for entry in std::fs::read_dir(source).expect("project directory is readable") {
        let entry = entry.expect("project entry is readable");
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if entry.path().is_dir() {
            if matches!(name.as_ref(), ".godot" | ".themosis" | "addons") {
                continue;
            }
            let nested = destination.join(name.as_ref());
            std::fs::create_dir_all(&nested).expect("project directory is created");
            copy_project(&entry.path(), &nested);
        } else {
            std::fs::copy(entry.path(), destination.join(name.as_ref()))
                .expect("project file is copied");
        }
    }
}

/// A throwaway copy of the checked-in component gallery, used by demo smoke
/// tests.
///
/// The copy registers the pinned extension library through a generated
/// `themosis.gdextension`, so a gallery run validates exactly the artifact the
/// suite was configured with — never the workspace path in the checked-in
/// developer manifest. Working in a copy also keeps the checked-in project and
/// a developer's import cache untouched, which makes the tests parallel-safe.
pub struct GalleryProject {
    directory: TempDir,
    logs: TempDir,
}

impl GalleryProject {
    /// Copies the gallery and wires it to the pinned library.
    ///
    /// Returns `None` when no extension library is available, like
    /// [`ProbeProject::new`].
    pub fn new() -> Option<Self> {
        let library = library()?;
        let project = Self {
            directory: TempDirBuilder::new()
                .prefix("themosis-gallery-test-")
                .tempdir()
                .expect("temporary Godot project is created"),
            logs: tempfile::tempdir().expect("Godot test log directory is created"),
        };
        copy_project(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/godot"),
            project.path(),
        );
        write_extension_manifest(project.path(), &library);
        Some(project)
    }

    /// Returns the temporary gallery project directory.
    pub fn path(&self) -> &Path {
        self.directory.path()
    }

    /// Returns a log path outside the project tree.
    pub fn log(&self, name: &str) -> PathBuf {
        self.logs.path().join(name)
    }

    /// Writes a project-relative file into the copy.
    pub fn write(&self, path: &str, contents: &str) {
        write_project_file(self.path(), path, contents);
    }

    /// Returns the imported theme artifacts, sorted.
    ///
    /// They only exist when the `.tms` importer actually ran, so their names
    /// prove the pinned library was loaded and performed the import.
    pub fn imported_themes(&self) -> Vec<String> {
        let mut names = std::fs::read_dir(self.path().join(".godot/imported"))
            .map(|entries| {
                entries
                    .map(|entry| {
                        entry
                            .expect("cache entry")
                            .file_name()
                            .to_string_lossy()
                            .into_owned()
                    })
                    .filter(|name| name.ends_with(".tres"))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    /// Runs one project script in a headless session.
    pub fn run_script(&self, executable: &str, script: &str, log_name: &str) -> Output {
        run_project_script(self.path(), &self.log(log_name), executable, script)
    }

    /// Imports the gallery so the extension registers and its assets convert.
    ///
    /// A copy starts without an import cache and needs two passes: the demo
    /// scene references the `.tms` themes while the first pass is still
    /// importing them. A genuine failure fails both passes and is reported.
    pub fn import(&self, executable: &str) -> Output {
        let flags = ["--headless", "--editor", "--import"];
        let first = self.run(executable, &flags, "gallery-import.log");
        if first.status.success() {
            return first;
        }
        self.run(executable, &flags, "gallery-import.log")
    }

    /// Runs the demo scene headless for a few frames.
    pub fn run_scene(&self, executable: &str) -> Output {
        self.run(
            executable,
            &["--headless", "--quit-after", "3"],
            "gallery-scene.log",
        )
    }

    /// Opens the gallery in an editor session that quits after a few frames.
    pub fn run_editor(&self, executable: &str) -> Output {
        self.run(
            executable,
            &["--headless", "--editor", "--quit-after", "3"],
            "gallery-editor.log",
        )
    }

    fn run(&self, executable: &str, flags: &[&str], log_name: &str) -> Output {
        Command::new(executable)
            .args(flags)
            .arg("--path")
            .arg(self.path())
            .arg("--log-file")
            .arg(self.log(log_name))
            .output()
            .expect("Godot gallery session starts")
    }
}
