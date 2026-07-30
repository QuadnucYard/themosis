//! Process-level tests for Godot validation and generation.

#![cfg(feature = "godot")]

use std::{path::Path, process::Command};

use serde_json::{Value, json};
use tempfile::{Builder as TempDirBuilder, TempDir};
use themosis_godot::{NATIVE_THEME_BUILDER_GDSCRIPT, NATIVE_THEME_RUNNER_GDSCRIPT};

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_themosis"))
}

#[test]
fn feature_exposes_godot_through_generic_commands() {
    for command_name in ["build", "check"] {
        let output = command()
            .args([command_name, "--help"])
            .output()
            .expect("CLI starts");
        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
        assert!(stdout.contains("--target <TARGET>"));
        assert!(stdout.contains("godot"));
        assert!(stdout.contains("--godot <FILE>"));
    }
}

#[test]
fn missing_godot_project_directory_is_rejected() {
    let project = TestProject::new();
    let missing_project = project.path().join("missing-project");

    let output = project
        .command_for_project(
            "check",
            "missing-godot-for-project-validation",
            &missing_project,
        )
        .arg(project.root())
        .output()
        .expect("CLI starts");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
    assert!(stderr.contains("cannot open Godot project directory"));
    assert!(stderr.contains("missing-project"));
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
    eprintln!("skipping runtime-backed CLI test: Godot is not installed");
    None
}

struct TestProject {
    directory: TempDir,
}

impl TestProject {
    fn new() -> Self {
        let directory = TempDirBuilder::new()
            .prefix("themosis-cli-test-")
            .tempdir()
            .expect("isolated Godot project is created");
        std::fs::write(
            directory.path().join("project.godot"),
            "[application]\nconfig/name=\"Themosis CLI test\"\n",
        )
        .expect("Godot project is written");
        std::fs::write(
            directory.path().join("tokens.json"),
            r#"{
                "background": {
                    "$type": "color",
                    "$value": { "colorSpace": "srgb", "components": [0.1, 0.2, 0.3], "alpha": 1.0 }
                },
                "font-size": {
                    "$type": "dimension",
                    "$value": { "value": 17, "unit": "px" }
                }
            }"#,
        )
        .expect("token fixture is written");
        let project = Self { directory };
        project.write_theme("Button", "normal");
        project
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

    fn root(&self) -> std::path::PathBuf {
        self.path().join("theme.kdl")
    }

    fn output(&self) -> std::path::PathBuf {
        self.path().join("generated/theme.tres")
    }

    fn write_theme(&self, target: &str, property: &str) {
        std::fs::write(
            self.root(),
            format!(
                r#"theme RuntimeBuild {{
                    tokens "tokens.json"
                    style Probe target="{target}" {{
                        token {property} "background"
                        token font_size "font-size"
                    }}
                }}"#,
            ),
        )
        .expect("theme fixture is written");
    }

    fn command(&self, command_name: &str, godot: &str) -> Command {
        self.command_for_project(command_name, godot, self.path())
    }

    fn command_for_project(&self, command_name: &str, godot: &str, project: &Path) -> Command {
        let mut command = command();
        command.args([
            command_name,
            "--target",
            "godot",
            "--godot",
            godot,
            "--project",
            project.to_str().expect("project path is UTF-8"),
        ]);
        command
    }

    fn check_command(&self, godot: &str) -> Command {
        let mut command = self.command("check", godot);
        command.arg(self.root());
        command
    }

    fn build_command(&self, godot: &str, output: &Path) -> Command {
        let mut command = self.command("build", godot);
        command
            .args(["--output", output.to_str().expect("output path is UTF-8")])
            .arg(self.root());
        command
    }
}

fn build(project: &TestProject, godot: &str) -> std::process::Output {
    let output = project.output();
    project
        .build_command(godot, &output)
        .output()
        .expect("CLI starts")
}

fn assert_generated_theme_loads(project: &TestProject, godot: &str) {
    std::fs::write(
        project.path().join("verify_theme.gd"),
        r#"
extends SceneTree
func _initialize() -> void:
    var generated := ResourceLoader.load("res://generated/theme.tres") as Theme
    if generated == null:
        quit(1)
        return
    var normal := generated.get_stylebox("normal", "Probe") as StyleBoxFlat
    if generated.get_font_size("font_size", "Probe") != 17 or normal == null:
        quit(1)
        return
    quit()
"#,
    )
    .expect("theme verification script is written");
    let output = Command::new(godot)
        .args(["--headless", "--path"])
        .arg(project.path())
        .arg("--log-file")
        .arg(project.path().join("verify-theme.log"))
        .args(["--script", "res://verify_theme.gd"])
        .output()
        .expect("Godot starts");
    assert!(
        output.status.success(),
        "Godot could not load the generated theme:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn run_native_builder(project: &TestProject, godot: &str, request: &Value) -> Value {
    let runner = TempDirBuilder::new()
        .prefix("themosis-native-builder-test-")
        .tempdir()
        .expect("native builder directory is created");
    let script = runner.path().join("native_theme_builder.gd");
    let entrypoint = runner.path().join("native_theme_runner.gd");
    let request_path = runner.path().join("request.json");
    let response_path = runner.path().join("response.json");
    std::fs::write(&script, NATIVE_THEME_BUILDER_GDSCRIPT).expect("native builder is written");
    std::fs::write(&entrypoint, NATIVE_THEME_RUNNER_GDSCRIPT).expect("native runner is written");
    std::fs::write(
        &request_path,
        serde_json::to_vec_pretty(request).expect("request is serializable"),
    )
    .expect("native builder request is written");

    let output = Command::new(godot)
        .args(["--headless", "--path"])
        .arg(project.path())
        .arg("--log-file")
        .arg(runner.path().join("godot.log"))
        .arg("--script")
        .arg(&entrypoint)
        .arg("--")
        .arg(&request_path)
        .arg(&response_path)
        .output()
        .expect("Godot starts");
    assert!(
        !output.status.success(),
        "malformed request unexpectedly succeeded: {}",
        String::from_utf8_lossy(&output.stderr),
    );
    serde_json::from_slice(
        &std::fs::read(response_path).expect("native builder response is written"),
    )
    .expect("native builder response is JSON")
}

#[test]
#[ignore = "not added"]
fn check_can_apply_portable_godot_backend_validation() {
    let Some(godot) = godot() else {
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/godot/theme/light.tms");

    let output = command()
        .args([
            "check",
            "--target",
            "godot",
            "--godot",
            &godot,
            root.to_str().expect("fixture path is UTF-8"),
        ])
        .output()
        .expect("CLI starts");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
    assert!(
        stdout.starts_with(
            "theme 'Application' sources and Godot mappings validate successfully with "
        )
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn build_generates_a_godot_theme_file() {
    let Some(godot) = godot() else {
        return;
    };
    let project = TestProject::new();

    let output = build(&project, &godot);

    assert!(
        output.status.success(),
        "generation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let source = std::fs::read_to_string(project.output()).expect("theme file was generated");
    assert!(source.starts_with("[gd_resource type=\"Theme\""));
    assert!(source.contains("Probe/base_type = &\"Button\""));
    assert!(source.contains("Probe/styles/normal = SubResource("));
    assert!(source.contains("Probe/font_sizes/font_size = 17"));

    let replacement = build(&project, &godot);
    assert!(replacement.status.success());
    assert_generated_theme_loads(&project, &godot);
}

#[test]
fn runtime_mapping_failure_is_structured_and_preserves_output() {
    let Some(godot) = godot() else {
        return;
    };
    let project = TestProject::new();
    let generated = build(&project, &godot);
    assert!(generated.status.success());
    let previous = std::fs::read_to_string(project.output()).expect("theme file was generated");
    project.write_theme("Button", "not_a_theme_item");

    let failed = build(&project, &godot);

    assert!(!failed.status.success());
    let stderr = String::from_utf8(failed.stderr).expect("stderr is UTF-8");
    assert!(
        stderr
            .contains("[unsupported_property style=Probe target=Button property=not_a_theme_item]")
    );
    assert_eq!(
        std::fs::read_to_string(project.output()).expect("previous output remains readable"),
        previous,
    );
}

#[test]
fn unknown_runtime_target_reports_structured_context() {
    let Some(godot) = godot() else {
        return;
    };
    let project = TestProject::new();
    project.write_theme("NotAGodotControl", "normal");

    let output = project.check_command(&godot).output().expect("CLI starts");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
    assert!(stderr.contains("[unknown_target style=Probe target=NotAGodotControl]"));
}

#[test]
fn color_rejects_a_non_flat_default_stylebox() {
    let Some(godot) = godot() else {
        return;
    };
    let project = TestProject::new();
    project.write_theme("HSeparator", "separator");

    let output = project.check_command(&godot).output().expect("CLI starts");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
    assert!(
        stderr.contains("[incompatible_stylebox style=Probe target=HSeparator property=separator]")
    );
    assert!(stderr.contains("a color can only modify StyleBoxFlat"));
}

#[test]
fn native_builder_rejects_malformed_candidate_and_integer_data() {
    let Some(godot) = godot() else {
        return;
    };
    let project = TestProject::new();
    let response = run_native_builder(
        &project,
        &godot,
        &json!({
            "operation": "check",
            "required_godot_version": null,
            "plan": {
                "schema_version": 2,
                "theme": "Malformed",
                "styles": [{
                    "name": "Probe",
                    "target": "Button",
                    "items": [
                        {
                            "property": "normal",
                            "state": null,
                            "value_kind": "color",
                            "candidates": ["not_a_category"],
                            "value": {"kind": "color", "rgba": [0.1, 0.2, 0.3, 1.0]}
                        },
                        {
                            "property": "font_size",
                            "state": null,
                            "value_kind": "dimension",
                            "candidates": ["font_size"],
                            "value": {"kind": "integer"}
                        }
                    ]
                }]
            }
        }),
    );

    assert_eq!(response["ok"], false);
    assert!(response["godot_version"].is_object());
    let diagnostics = response["diagnostics"]
        .as_array()
        .expect("diagnostics are an array");
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0]["code"], "invalid_plan");
    assert_eq!(diagnostics[0]["style"], "Probe");
    assert_eq!(diagnostics[0]["target"], "Button");
    assert_eq!(diagnostics[0]["property"], "normal");
    assert_eq!(diagnostics[1]["code"], "invalid_integer");
    assert_eq!(diagnostics[1]["property"], "font_size");
}

#[test]
fn exact_version_mismatch_preserves_existing_output() {
    let Some(godot) = godot() else {
        return;
    };
    let project = TestProject::new();
    std::fs::create_dir(project.output().parent().expect("output has a parent"))
        .expect("output directory is created");
    let previous = "previous theme output\n";
    std::fs::write(project.output(), previous).expect("previous output is written");

    let output = project
        .command("build", &godot)
        .args(["--require-godot-version", "0.0.0", "--output"])
        .arg(project.output())
        .arg(project.root())
        .output()
        .expect("CLI starts");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
    assert!(stderr.contains("[godot_version_mismatch] required Godot 0.0.0, got"));
    assert_eq!(
        std::fs::read_to_string(project.output()).expect("previous output remains readable"),
        previous,
    );
}

#[test]
fn output_escape_is_rejected_without_creating_directories() {
    let project = TestProject::new();
    let outside = TempDirBuilder::new()
        .prefix("themosis-cli-outside-")
        .tempdir()
        .expect("outside directory is created");
    let output = outside.path().join("new/directory/theme.tres");

    let result = project
        .command("build", "missing-godot-for-output-validation")
        .args(["--output", output.to_str().expect("output path is UTF-8")])
        .arg(project.root())
        .output()
        .expect("CLI starts");

    assert!(!result.status.success());
    let stderr = String::from_utf8(result.stderr).expect("stderr is UTF-8");
    assert!(stderr.contains("escapes project"));
    assert!(!outside.path().join("new").exists());
}

#[cfg(unix)]
#[test]
fn output_parent_symlink_escape_is_rejected_by_the_command() {
    use std::os::unix::fs::symlink;

    let project = TestProject::new();
    let outside = TempDirBuilder::new()
        .prefix("themosis-cli-outside-")
        .tempdir()
        .expect("outside directory is created");
    symlink(outside.path(), project.path().join("linked")).expect("escape symlink is created");

    let result = project
        .command("build", "missing-godot-for-output-validation")
        .args(["--output", "res://linked/theme.tres"])
        .arg(project.root())
        .output()
        .expect("CLI starts");

    assert!(!result.status.success());
    let stderr = String::from_utf8(result.stderr).expect("stderr is UTF-8");
    assert!(stderr.contains("escapes project"));
    assert!(!outside.path().join("theme.tres").exists());
}

#[cfg(unix)]
#[test]
fn godot_timeout_stops_a_stalled_runtime() {
    use std::{os::unix::fs::PermissionsExt as _, time::Instant};

    let project = TestProject::new();
    let executable = project.path().join("stalled-godot");
    std::fs::write(&executable, "#!/bin/sh\nexec sleep 5\n").expect("fake Godot is written");
    let mut permissions = std::fs::metadata(&executable)
        .expect("fake Godot metadata is readable")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&executable, permissions).expect("fake Godot is executable");

    let started = Instant::now();
    let output = project
        .command(
            "check",
            executable.to_str().expect("executable path is UTF-8"),
        )
        .args(["--godot-timeout", "1"])
        .arg(project.root())
        .output()
        .expect("CLI starts");

    assert!(!output.status.success());
    assert!(started.elapsed().as_secs() < 4);
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
    assert!(stderr.contains("Godot target operation timed out"));
    assert!(stderr.contains("stdout:\n"));
    assert!(stderr.contains("stderr:\n"));
    assert!(stderr.contains("Godot log:\n"));
}
