//! Process-level tests for source validation.

use std::{path::Path, process::Command};

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_themosis"))
}

#[test]
fn check_reports_a_valid_theme_on_stdout() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../themosis/tests/fixtures/valid/theme.kdl");

    let output = command()
        .args(["check", root.to_str().expect("fixture path is UTF-8")])
        .output()
        .expect("CLI starts");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout is UTF-8"),
        "theme 'Application' sources compile successfully\n"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn check_reports_source_failures_on_stderr() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/invalid.kdl");

    let output = command()
        .args(["check", root.to_str().expect("fixture path is UTF-8")])
        .output()
        .expect("CLI starts");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
    assert!(stderr.starts_with("themosis: failed to read 'missing.tokens.json':"));
}

#[test]
fn invalid_invocation_has_a_distinct_usage_exit_code() {
    let output = command().output().expect("CLI starts");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
    assert!(stderr.starts_with("Themosis command-line interface\n"));
    assert!(stderr.contains("Usage: themosis"));
}

#[cfg(not(feature = "godot"))]
#[test]
fn godot_target_is_absent_without_the_feature() {
    let output = command().arg("--help").output().expect("CLI starts");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
    assert!(stdout.contains("check"));
    assert!(stdout.contains("build"));

    for command_name in ["build", "check"] {
        let output = command()
            .args([command_name, "--help"])
            .output()
            .expect("CLI starts");
        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
        assert!(!stdout.contains("--godot"));
        assert!(!stdout.contains("--project"));
    }

    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../themosis/tests/fixtures/valid/theme.kdl");
    let output = command()
        .args([
            "check",
            "--target",
            "godot",
            root.to_str().expect("fixture path is UTF-8"),
        ])
        .output()
        .expect("CLI starts");
    assert_eq!(output.status.code(), Some(2));

    let output = command()
        .args([
            "build",
            "--target",
            "godot",
            "--output",
            "theme.tres",
            root.to_str().expect("fixture path is UTF-8"),
        ])
        .output()
        .expect("CLI starts");
    assert_eq!(output.status.code(), Some(2));
}
