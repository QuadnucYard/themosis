//! Native CLI runner tests against a temporary addon project.

use themosis_godot::{RUNNER_SCHEMA_VERSION, RunnerOperation, RunnerRequest};

mod support;

#[test]
fn native_cli_runner_checks_the_probe_theme() {
    let Some(executable) = support::godot() else {
        return;
    };
    let Some(project) = support::ProbeProject::new() else {
        return;
    };
    project.write_theme_fixture();

    let request = RunnerRequest {
        schema_version: RUNNER_SCHEMA_VERSION,
        operation: RunnerOperation::Check,
        source: "res://theme/probe.tms".to_owned(),
        output: None,
        required_godot_version: None,
    };
    let (status, response) = project.run_main_loop(&executable, &request, "runner.log");
    assert!(
        status.success() && response.ok,
        "native runner check failed: {:?}",
        response.diagnostics
    );
}

#[test]
fn native_cli_runner_enforces_exact_versions() {
    let Some(executable) = support::godot() else {
        return;
    };
    let Some(project) = support::ProbeProject::new() else {
        return;
    };
    project.write_theme_fixture();

    let request = RunnerRequest {
        schema_version: RUNNER_SCHEMA_VERSION,
        operation: RunnerOperation::Check,
        source: "res://theme/probe.tms".to_owned(),
        output: None,
        required_godot_version: Some("0.0.0".to_owned()),
    };
    let (_, response) = project.run_main_loop(&executable, &request, "runner-version.log");
    assert!(!response.ok);
    assert_eq!(response.diagnostics[0].code, "godot_version_mismatch");
}

/// An imported fragment that is a symlink outside the project must be
/// rejected: Godot's `FileAccess` follows links transparently, so the source
/// provider has to confine physical reads itself.
#[cfg(unix)]
#[test]
fn native_cli_runner_rejects_imported_source_symlink_escapes() {
    use std::os::unix::fs::symlink;

    let Some(executable) = support::godot() else {
        return;
    };
    let Some(project) = support::ProbeProject::new() else {
        return;
    };
    let outside = tempfile::tempdir().expect("outside directory is created");
    std::fs::write(
        outside.path().join("fragment.kdl"),
        "style Probe target=Button {\n    number font_size 17\n}\n",
    )
    .expect("outside fragment is written");
    symlink(
        outside.path().join("fragment.kdl"),
        project.path().join("fragment.kdl"),
    )
    .expect("escape symlink is created");
    project.write(
        "theme.kdl",
        "theme RuntimeBuild {\n    import \"fragment.kdl\"\n}\n",
    );

    let request = RunnerRequest {
        schema_version: RUNNER_SCHEMA_VERSION,
        operation: RunnerOperation::Check,
        source: "res://theme.kdl".to_owned(),
        output: None,
        required_godot_version: None,
    };
    let (status, response) = project.run_main_loop(&executable, &request, "runner-escape.log");
    assert!(
        !response.ok && !status.success(),
        "an imported source that escapes the project must fail"
    );
    let message = response
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        message.contains("outside the Godot project"),
        "unexpected diagnostics: {message}"
    );
}
