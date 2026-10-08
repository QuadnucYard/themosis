//! Profile and build-runner tests against a temporary project.

mod support;

use support::ProbeProject;

/// Printed by the native build runner for every generated theme.
const BUILD_GENERATED: &str = "Themosis: generated ";

/// A legacy migration that no longer validates must fail the build without
/// persisting the invalid configuration.
#[test]
fn build_runner_rejects_invalid_legacy_migration_without_persisting() {
    let Some(executable) = support::godot() else {
        return;
    };
    let Some(project) = ProbeProject::new() else {
        return;
    };
    project.write_theme_fixture();
    let settings =
        std::fs::read_to_string(project.path().join("project.godot")).expect("settings are read");
    project.write(
        "project.godot",
        &format!(
            "{settings}\n[themosis]\ntheme_source=\"res://theme/probe.tms\"\ngenerated_theme=\"res://../outside.tres\"\n"
        ),
    );

    let run = project.run_build_runner(&executable, &["--all"], "legacy-migration.log");
    let messages = support::messages(&run);
    assert!(
        !run.status.success(),
        "a rejected migration must fail the build:\n{messages}"
    );
    assert!(
        !project.path().join("themosis.godot.json").exists(),
        "a rejected migration must not persist configuration:\n{messages}"
    );
    assert!(
        messages.contains("invalid migrated profile configuration"),
        "unexpected failure:\n{messages}"
    );
}

/// Profile configuration mixing a working, a failing, and a disabled profile.
const MIXED_PROFILE_CONFIG: &str = r#"{
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
      "source": "res://theme/missing.tms"
    },
    {
      "auto_refresh": false,
      "build_on_start": false,
      "enabled": false,
      "name": "gamma",
      "output": "res://.themosis/materialized/gamma.tres",
      "preview": "none",
      "source": "res://theme/missing.tms"
    }
  ],
  "version": 1
}
"#;

#[test]
fn addon_build_script_selects_profiles_and_rejects_invalid_arguments() {
    let Some(executable) = support::godot() else {
        return;
    };
    let Some(project) = ProbeProject::new() else {
        return;
    };
    project.write_profile_fixtures();
    project.write_profile_config();

    let output = project.run_build_runner(&executable, &["--profile", "alpha"], "build-alpha.log");
    let text = support::messages(&output);
    assert!(
        output.status.success() && !text.contains("SCRIPT ERROR") && !text.contains("Parse Error"),
        "--profile run failed:\n{text}"
    );
    assert!(
        text.contains(BUILD_GENERATED) && text.contains("res://.themosis/materialized/alpha.tres"),
        "--profile run did not report its generated theme:\n{text}"
    );

    let output = project.run_build_runner(&executable, &["--all"], "build-all.log");
    let text = support::messages(&output);
    assert!(
        output.status.success() && !text.contains("SCRIPT ERROR") && !text.contains("Parse Error"),
        "--all run failed:\n{text}"
    );
    assert!(
        text.contains("res://.themosis/materialized/alpha.tres")
            && text.contains("res://.themosis/materialized/beta.tres"),
        "--all run did not report every generated theme:\n{text}"
    );

    let output = project.run_build_runner(&executable, &["--bogus"], "build-invalid.log");
    let text = support::messages(&output);
    assert_eq!(
        output.status.code(),
        Some(2),
        "invalid arguments must exit 2:\n{text}"
    );
    assert!(
        text.contains(
            "usage: godot --headless --path . --main-loop ThemosisBuildRunner -- --profile NAME"
        ),
        "invalid arguments must print usage:\n{text}"
    );
    assert!(
        !text.contains("SCRIPT ERROR") && !text.contains("Parse Error"),
        "the build runner must not error:\n{text}"
    );
}

#[test]
fn build_runner_reports_every_failure_and_rejects_invalid_configuration() {
    let Some(executable) = support::godot() else {
        return;
    };
    let Some(project) = ProbeProject::new() else {
        return;
    };
    project.write_profile_fixtures();
    project.write("themosis.godot.json", MIXED_PROFILE_CONFIG);

    // A batch reports every enabled profile: the working one is materialized
    // and reported, the failing one keeps its message, and `--all` exits
    // nonzero instead of stopping at the first failure.
    let output = project.run_build_runner(&executable, &["--all"], "build-mixed.log");
    let text = support::messages(&output);
    assert_eq!(
        output.status.code(),
        Some(1),
        "a failing profile must exit 1:\n{text}"
    );
    assert!(
        text.contains("Themosis[alpha]: compiled res://theme/alpha.tms -> res://.themosis/materialized/alpha.tres"),
        "--all must report each successful profile:\n{text}"
    );
    assert!(
        text.contains(BUILD_GENERATED) && text.contains("res://.themosis/materialized/alpha.tres"),
        "--all must report the generated artifact:\n{text}"
    );
    assert!(
        text.contains("Themosis[beta]: ") && text.contains("res://theme/missing.tms"),
        "--all must report the failing profile's own error:\n{text}"
    );
    assert!(
        !text.contains("res://.themosis/materialized/beta.tres"),
        "a failed profile must not claim an output:\n{text}"
    );
    assert!(
        !text.contains("gamma"),
        "disabled profiles must not run:\n{text}"
    );
    assert!(
        project
            .path()
            .join(".themosis/materialized/alpha.tres")
            .is_file()
            && !project
                .path()
                .join(".themosis/materialized/beta.tres")
                .exists(),
        "only the working profile may be materialized"
    );

    project.write("themosis.godot.json", "{ not json }");
    let output = project.run_build_runner(&executable, &["--all"], "build-config.log");
    let text = support::messages(&output);
    assert_eq!(
        output.status.code(),
        Some(1),
        "a malformed profile file must exit 1:\n{text}"
    );
    assert!(
        text.contains("Themosis: cannot parse profile configuration"),
        "a malformed profile file must report its parse failure:\n{text}"
    );
}
