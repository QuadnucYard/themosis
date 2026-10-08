//! Process-level tests for project-aware Godot validation and generation.

#![cfg(feature = "godot")]

use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

use tempfile::{Builder as TempDirBuilder, TempDir};

/// Token fixture shared by the generated themes.
const TOKENS: &str = r#"{
    "background": {
        "$type": "color",
        "$value": { "colorSpace": "srgb", "components": [0.1, 0.2, 0.3], "alpha": 1.0 }
    },
    "font-size": {
        "$type": "dimension",
        "$value": { "value": 17, "unit": "px" }
    }
}"#;

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_themosis"))
}

/// Turns missing prerequisites into failures; set by the Godot CI job.
const REQUIRE_ENV: &str = "THEMOSIS_REQUIRE_GODOT";

/// Reports a missing prerequisite, failing when the environment requires the
/// suite to run.
fn skip(prerequisite: &str, hint: &str) {
    assert!(
        !std::env::var_os(REQUIRE_ENV).is_some_and(|value| value != "0"),
        "required for this run, but missing: {prerequisite}; {hint}"
    );
    eprintln!("skipping runtime-backed CLI test: {prerequisite} is missing; {hint}");
}

fn godot() -> Option<String> {
    if let Some(executable) = std::env::var_os("THEMOSIS_GODOT_BINARY") {
        return Some(executable.to_string_lossy().into_owned());
    }
    for executable in ["godot", "godot4"] {
        if Command::new(executable).arg("--version").output().is_ok() {
            return Some(executable.to_owned());
        }
    }
    skip("Godot", "install Godot 4.5+ or set THEMOSIS_GODOT_BINARY");
    None
}

/// Locates the GDExtension library the runtime tests load.
///
/// `THEMOSIS_GODOT_LIBRARY` pins the artifact built for these tests; without it
/// the path is derived from this build's profile.
fn built_plugin_library() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("THEMOSIS_GODOT_LIBRARY") {
        let path = PathBuf::from(path);
        assert!(
            path.is_file(),
            "THEMOSIS_GODOT_LIBRARY is not a file: {}",
            path.display()
        );
        return Some(path);
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
    path.is_file().then_some(path)
}

/// Path of the maintained probe project that loads the GDExtension.
fn probe_project() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/themosis-godot-plugin/tests/godot")
}

struct TestProject {
    directory: TempDir,
}

impl TestProject {
    /// Creates an empty project without the Themosis addon.
    fn minimal() -> Self {
        let directory = TempDirBuilder::new()
            .prefix("themosis-cli-test-")
            .tempdir()
            .expect("isolated Godot project is created");
        std::fs::write(
            directory.path().join("project.godot"),
            "[application]\nconfig/name=\"Themosis CLI test\"\n",
        )
        .expect("Godot project is written");
        std::fs::write(directory.path().join("tokens.json"), TOKENS)
            .expect("token fixture is written");
        let project = Self { directory };
        project.write_theme("Button", "normal");
        project
    }

    /// Creates a temporary copy of the probe project wired to the built
    /// extension library.
    ///
    /// The project is registered by construction: the manifest and
    /// `.godot/extension_list.cfg` are written directly, so the addon is
    /// discoverable without an editor import pass.
    fn with_extension() -> Option<Self> {
        let library = built_plugin_library().or_else(|| {
            skip(
                "the GDExtension library",
                "run `just test-godot` to build it",
            );
            None
        })?;
        let directory = TempDirBuilder::new()
            .prefix("themosis-cli-test-")
            .tempdir()
            .expect("isolated Godot project is created");
        for file in ["project.godot", "main.tscn"] {
            std::fs::copy(probe_project().join(file), directory.path().join(file))
                .unwrap_or_else(|error| panic!("probe file '{file}' is copied: {error}"));
        }
        let project = Self { directory };
        project.write_manifest(&library);
        std::fs::create_dir_all(project.path().join(".godot"))
            .expect("Godot metadata directory is created");
        std::fs::write(
            project.path().join(".godot/extension_list.cfg"),
            "res://themosis.gdextension\n",
        )
        .expect("extension registration is written");
        std::fs::write(project.path().join("tokens.json"), TOKENS)
            .expect("token fixture is written");
        project.write_theme("Button", "normal");
        Some(project)
    }

    fn path(&self) -> &Path {
        let path = self.directory.path();
        assert!(path.is_dir(), "Godot project directory is missing");
        assert!(
            path.join("project.godot").is_file(),
            "Godot project directory has no project.godot"
        );
        path
    }

    fn root(&self) -> PathBuf {
        self.path().join("theme.kdl")
    }

    fn output(&self) -> PathBuf {
        self.path().join("generated/theme.tres")
    }

    fn write_manifest(&self, library: &Path) {
        let library = library
            .to_str()
            .expect("extension library path is UTF-8")
            .to_owned();
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
            self.path().join("themosis.gdextension"),
            format!(
                "[configuration]\nentry_symbol = \"gdext_rust_init\"\ncompatibility_minimum = 4.5\nreloadable = true\n\n[libraries]\n{libraries}"
            ),
        )
        .expect("extension manifest is written");
    }

    /// Writes a manifest and extension registration without a real library.
    fn write_stub_addon(&self) {
        self.write_manifest(Path::new("stub.gdextension"));
        std::fs::create_dir_all(self.path().join(".godot")).expect("import directory is created");
        std::fs::write(
            self.path().join(".godot/extension_list.cfg"),
            "res://themosis.gdextension\n",
        )
        .expect("stub extension registration is written");
    }

    fn write_theme(&self, target: &str, property: &str) {
        std::fs::write(
            self.root(),
            format!(
                "theme RuntimeBuild {{\n    tokens \"tokens.json\"\n    style Probe target=\"{target}\" {{\n        token {property} \"background\"\n        token font_size \"font-size\"\n    }}\n}}\n",
            ),
        )
        .expect("theme fixture is written");
    }

    fn write_resource_theme(&self, target: &str, property: &str, reference: &str) {
        std::fs::write(
            self.root(),
            format!(
                "theme RuntimeBuild {{\n    tokens \"tokens.json\"\n    style Probe target=\"{target}\" {{\n        resource {property} \"{reference}\"\n    }}\n}}\n",
            ),
        )
        .expect("resource theme fixture is written");
    }

    fn godot_command(&self, action: &str, godot: &str) -> Command {
        let mut command = command();
        command.args(["godot", action, "--godot", godot, "--project"]);
        command.arg(self.path());
        command
    }

    fn check(&self, godot: &str) -> Output {
        self.godot_command("check", godot)
            .arg(self.root())
            .output()
            .expect("CLI starts")
    }

    fn build(&self, godot: &str, output: &Path) -> Output {
        self.godot_command("build", godot)
            .args(["--output"])
            .arg(output)
            .arg(self.root())
            .output()
            .expect("CLI starts")
    }
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is UTF-8")
}

#[test]
fn output_escape_is_rejected_without_creating_directories() {
    let project = TestProject::minimal();
    let outside = TempDirBuilder::new()
        .prefix("themosis-cli-outside-")
        .tempdir()
        .expect("outside directory is created");
    let output = outside.path().join("new/directory/theme.tres");

    let result = project
        .godot_command("build", "missing-godot-for-output-validation")
        .args(["--output", output.to_str().expect("output path is UTF-8")])
        .arg(project.root())
        .output()
        .expect("CLI starts");

    assert!(!result.status.success());
    assert!(stderr(&result).contains("escapes project"));
    assert!(!outside.path().join("new").exists());
}

#[cfg(unix)]
#[test]
fn output_parent_symlink_escape_is_rejected_by_the_command() {
    use std::os::unix::fs::symlink;

    let project = TestProject::minimal();
    let outside = TempDirBuilder::new()
        .prefix("themosis-cli-outside-")
        .tempdir()
        .expect("outside directory is created");
    symlink(outside.path(), project.path().join("linked")).expect("escape symlink is created");

    let result = project
        .godot_command("build", "missing-godot-for-output-validation")
        .args(["--output", "res://linked/theme.tres"])
        .arg(project.root())
        .output()
        .expect("CLI starts");

    assert!(!result.status.success());
    assert!(stderr(&result).contains("escapes project"));
    assert!(!outside.path().join("theme.tres").exists());
}

#[test]
fn missing_project_directory_is_rejected() {
    let project = TestProject::minimal();
    let missing = project.path().join("missing-project");

    let result = command()
        .args([
            "godot",
            "check",
            "--godot",
            "missing-godot-for-project-validation",
            "--project",
        ])
        .arg(&missing)
        .arg(project.root())
        .output()
        .expect("CLI starts");

    assert!(!result.status.success());
    let message = stderr(&result);
    assert!(message.contains("cannot open Godot project directory"));
    assert!(message.contains("missing-project"));
}

#[test]
fn missing_addon_reports_an_actionable_error() {
    let project = TestProject::minimal();

    let output = project.check("missing-godot-for-addon-validation");

    assert!(!output.status.success());
    let message = stderr(&output);
    assert!(message.contains("has no Themosis addon"));
    assert!(message.contains("res://addons/themosis"));
}

#[test]
fn unimported_project_reports_an_actionable_error() {
    let project = TestProject::minimal();
    // The addon is installed but Godot has not imported the project yet.
    project.write_manifest(Path::new("stub.gdextension"));

    let output = project.check("missing-godot-for-import-validation");

    assert!(!output.status.success());
    let message = stderr(&output);
    assert!(message.contains("has not registered the Themosis addon"));
    assert!(message.contains("--editor --import"));
}

#[cfg(unix)]
#[test]
fn godot_timeout_stops_a_stalled_runtime() {
    use std::{os::unix::fs::PermissionsExt as _, time::Instant};

    let project = TestProject::minimal();
    project.write_stub_addon();
    let executable = project.path().join("stalled-godot");
    std::fs::write(&executable, "#!/bin/sh\nexec sleep 5\n").expect("fake Godot is written");
    let mut permissions = std::fs::metadata(&executable)
        .expect("fake Godot metadata is readable")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&executable, permissions).expect("fake Godot is executable");

    let started = Instant::now();
    let result = project
        .godot_command("check", executable.to_str().expect("path is UTF-8"))
        .args(["--timeout", "1"])
        .arg(project.root())
        .output()
        .expect("CLI starts");

    assert!(!result.status.success());
    assert!(started.elapsed().as_secs() < 4);
    let message = stderr(&result);
    assert!(message.contains("Godot runner timed out"));
    assert!(message.contains("Godot log:"));
}

#[cfg(unix)]
#[test]
fn protocol_mismatch_is_rejected() {
    use std::os::unix::fs::PermissionsExt as _;

    let project = TestProject::minimal();
    project.write_stub_addon();
    let executable = project.path().join("fake-godot");
    std::fs::write(
        &executable,
        r#"#!/bin/sh
resp=""
for arg in "$@"; do resp="$arg"; done
printf '%s' '{"schema_version":999,"ok":true,"godot_version":{"display":"fake","major":9,"minor":9,"patch":9,"status":"","build":"","hash":""}}' > "$resp"
exit 0
"#,
    )
    .expect("fake Godot is written");
    let mut permissions = std::fs::metadata(&executable)
        .expect("fake Godot metadata is readable")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&executable, permissions).expect("fake Godot is executable");

    let result = project
        .godot_command("check", executable.to_str().expect("path is UTF-8"))
        .arg(project.root())
        .output()
        .expect("CLI starts");

    assert!(!result.status.success());
    assert!(stderr(&result).contains("runner protocol version 999"));
}

#[test]
fn check_validates_a_project_theme_by_inference() {
    let Some(godot) = godot() else {
        return;
    };
    let Some(project) = TestProject::with_extension() else {
        return;
    };

    // No --project: the project is inferred from the source's ancestors.
    let output = command()
        .args(["godot", "check", "--godot", &godot])
        .arg(project.root())
        .output()
        .expect("CLI starts");

    assert!(
        output.status.success(),
        "check failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
    assert!(stdout.contains("validate successfully with"));
    assert!(output.stderr.is_empty());
}

#[test]
fn check_accepts_res_relative_sources() {
    let Some(godot) = godot() else {
        return;
    };
    let Some(project) = TestProject::with_extension() else {
        return;
    };

    let output = project
        .godot_command("check", &godot)
        .arg("res://theme.kdl")
        .output()
        .expect("CLI starts");

    assert!(
        output.status.success(),
        "check failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
#[test]
fn res_relative_symlink_escape_is_rejected() {
    use std::os::unix::fs::symlink;

    let project = TestProject::minimal();
    let outside = TempDirBuilder::new()
        .prefix("themosis-cli-outside-")
        .tempdir()
        .expect("outside directory is created");
    std::fs::write(outside.path().join("escaped.kdl"), "theme Escaped {}\n")
        .expect("outside theme is written");
    symlink(outside.path(), project.path().join("linked")).expect("escape symlink is created");

    let output = project
        .godot_command("check", "godot")
        .arg("res://linked/escaped.kdl")
        .output()
        .expect("CLI starts");

    assert!(!output.status.success());
    let message = stderr(&output);
    assert!(
        message.contains("outside Godot project"),
        "unexpected error: {message}"
    );
}

#[test]
fn build_writes_a_loadable_theme_without_runtime_dependency() {
    let Some(godot) = godot() else {
        return;
    };
    let Some(project) = TestProject::with_extension() else {
        return;
    };
    let generated = project.build(&godot, &project.output());
    assert!(
        generated.status.success(),
        "build failed:\n{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    let source = std::fs::read_to_string(project.output()).expect("theme file was generated");
    assert!(source.starts_with("[gd_resource type=\"Theme\""));
    assert!(source.contains("Probe/base_type = &\"Button\""));
    assert!(source.contains("Probe/font_sizes/font_size = 17"));

    // A clean project without the addon can load the artifact.
    let clean = TestProject::minimal();
    std::fs::create_dir_all(clean.path().join("generated")).expect("output directory is created");
    std::fs::copy(project.output(), clean.path().join("generated/theme.tres"))
        .expect("theme is copied into the clean project");
    std::fs::write(
        clean.path().join("verify.gd"),
        "extends SceneTree\nfunc _initialize() -> void:\n\tvar theme := ResourceLoader.load(\"res://generated/theme.tres\") as Theme\n\tif theme == null:\n\t\tquit(1)\n\t\treturn\n\tif theme.get_font_size(\"font_size\", \"Probe\") != 17:\n\t\tquit(1)\n\t\treturn\n\tquit()\n",
    )
    .expect("verification script is written");
    let verification = Command::new(&godot)
        .args(["--headless", "--path"])
        .arg(clean.path())
        .arg("--log-file")
        .arg(clean.path().join("verify.log"))
        .args(["--script", "res://verify.gd"])
        .output()
        .expect("Godot starts");
    assert!(
        verification.status.success(),
        "generated theme did not load without the addon:\n{}\n{}",
        String::from_utf8_lossy(&verification.stdout),
        String::from_utf8_lossy(&verification.stderr),
    );
}

#[test]
fn unknown_target_is_reported_with_context() {
    let Some(godot) = godot() else {
        return;
    };
    let Some(project) = TestProject::with_extension() else {
        return;
    };
    project.write_theme("NotAGodotControl", "normal");

    let output = project.check(&godot);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("[unknown_target style=Probe target=NotAGodotControl]"));
}

#[test]
fn unsupported_property_is_reported_with_context() {
    let Some(godot) = godot() else {
        return;
    };
    let Some(project) = TestProject::with_extension() else {
        return;
    };
    project.write_theme("Button", "not_a_theme_item");

    let output = project.check(&godot);

    assert!(!output.status.success());
    let message = stderr(&output);
    assert!(
        message
            .contains("[unsupported_property style=Probe target=Button property=not_a_theme_item]")
    );
    assert!(message.contains("has no compatible color item"));
}

#[test]
fn color_rejects_a_non_flat_default_stylebox() {
    let Some(godot) = godot() else {
        return;
    };
    let Some(project) = TestProject::with_extension() else {
        return;
    };
    project.write_theme("HSeparator", "separator");

    let output = project.check(&godot);

    assert!(!output.status.success());
    let message = stderr(&output);
    assert!(
        message
            .contains("[incompatible_stylebox style=Probe target=HSeparator property=separator]")
    );
    assert!(message.contains("a color can only modify StyleBoxFlat"));
}

#[test]
fn resource_type_mismatch_is_reported() {
    let Some(godot) = godot() else {
        return;
    };
    let Some(project) = TestProject::with_extension() else {
        return;
    };
    std::fs::write(
        project.path().join("not_a_font.tres"),
        "[gd_resource type=\"StyleBoxFlat\" format=3]\n\n[resource]\nbg_color = Color(1, 0, 0, 1)\n",
    )
    .expect("non-font resource is written");
    project.write_resource_theme("Label", "font", "res://not_a_font.tres");

    let output = project.check(&godot);

    assert!(!output.status.success());
    let message = stderr(&output);
    assert!(message.contains("[resource_type style=Probe target=Label property=font]"));
    assert!(message.contains("must inherit Font"));
}

#[test]
fn exact_version_mismatch_preserves_existing_output() {
    let Some(godot) = godot() else {
        return;
    };
    let Some(project) = TestProject::with_extension() else {
        return;
    };
    let output_path = project.output();
    std::fs::create_dir_all(output_path.parent().expect("output has a parent"))
        .expect("output directory is created");
    let previous = "previous theme output\n";
    std::fs::write(&output_path, previous).expect("previous output is written");

    let result = project
        .godot_command("build", &godot)
        .args(["--require-version", "0.0.0", "--output"])
        .arg(&output_path)
        .arg(project.root())
        .output()
        .expect("CLI starts");

    assert!(!result.status.success());
    assert!(stderr(&result).contains("[godot_version_mismatch]"));
    assert_eq!(
        std::fs::read_to_string(&output_path).expect("previous output remains readable"),
        previous,
    );
}

#[test]
fn runtime_mapping_failure_is_structured_and_preserves_output() {
    let Some(godot) = godot() else {
        return;
    };
    let Some(project) = TestProject::with_extension() else {
        return;
    };
    let generated = project.build(&godot, &project.output());
    assert!(
        generated.status.success(),
        "build failed:\n{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    let previous = std::fs::read_to_string(project.output()).expect("theme file was generated");
    project.write_theme("Button", "not_a_theme_item");

    let failed = project.build(&godot, &project.output());

    assert!(!failed.status.success());
    assert!(
        stderr(&failed)
            .contains("[unsupported_property style=Probe target=Button property=not_a_theme_item]")
    );
    assert_eq!(
        std::fs::read_to_string(project.output()).expect("previous output remains readable"),
        previous,
    );
}

#[test]
fn cli_checks_without_an_application_scene() {
    let Some(godot) = godot() else {
        return;
    };
    let Some(project) = TestProject::with_extension() else {
        return;
    };
    std::fs::write(
        project.path().join("project.godot"),
        "[application]\nconfig/name=\"Themosis CLI test\"\n",
    )
    .expect("scene-free project");
    let checked = project
        .godot_command("check", &godot)
        // A cold project start (engine boot, extension load, first scan) can
        // exceed a few seconds on a loaded machine; timeout enforcement itself
        // is covered by `godot_timeout_stops_a_stalled_runtime`.
        .args(["--timeout", "30"])
        .arg(project.root())
        .output()
        .expect("CLI starts");
    assert!(checked.status.success(), "{}", stderr(&checked));
    assert!(project.build(&godot, &project.output()).status.success());
}

#[test]
fn cli_preserves_kdl_source_spans() {
    let Some(godot) = godot() else {
        return;
    };
    let Some(project) = TestProject::with_extension() else {
        return;
    };
    std::fs::write(
        project.root(),
        "theme Broken {\n style Probe target=Button {\n number font_size [\n }\n}\n",
    )
    .expect("invalid source");
    let result = project.check(&godot);
    assert!(!result.status.success());
    assert!(stderr(&result).contains("at bytes"), "{}", stderr(&result));
}

#[test]
fn cli_replacement_does_not_consume_another_output() {
    let Some(godot) = godot() else {
        return;
    };
    let Some(project) = TestProject::with_extension() else {
        return;
    };
    let sibling = project.path().join("generated/theme.themosis-tmp.tres");
    assert!(project.build(&godot, &sibling).status.success());
    let contents = std::fs::read(&sibling).expect("first output");
    assert!(project.build(&godot, &project.output()).status.success());
    assert_eq!(
        std::fs::read(&sibling).expect("first output survives"),
        contents
    );
    assert!(project.build(&godot, &project.output()).status.success());
    assert_eq!(
        std::fs::read(&sibling).expect("first output survives replacement"),
        contents
    );
}
