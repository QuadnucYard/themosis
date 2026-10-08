//! Importer lifecycle tests against a temporary addon project.

mod support;

use support::ProbeProject;

#[test]
fn headless_import_refreshes_dependencies_and_rejects_invalid_edits() {
    let Some(executable) = support::godot() else {
        return;
    };
    let Some(project) = ProbeProject::new() else {
        return;
    };
    project.write_theme_fixture();
    let first = import(&project, &executable, "first.log");
    assert!(
        first.status.success() && !support::messages(&first).contains("SCRIPT ERROR"),
        "{}",
        support::messages(&first)
    );
    let cache = project.imported_theme_path().expect("cached theme");
    let previous = std::fs::read(&cache).expect("first theme");
    let modified = std::fs::metadata(&cache)
        .expect("cache metadata")
        .modified()
        .expect("timestamp");
    let unchanged = import(&project, &executable, "unchanged.log");
    assert!(
        unchanged.status.success(),
        "{}",
        support::messages(&unchanged)
    );
    assert_eq!(
        std::fs::metadata(&cache)
            .expect("cache metadata")
            .modified()
            .expect("timestamp"),
        modified,
        "unchanged imports must not rebuild native resources"
    );
    project.write("theme/tokens.json", r#"{"background":{"$type":"color","$value":{"colorSpace":"srgb","components":[0.9,0.2,0.3],"alpha":1}}}"#);
    let changed = import(&project, &executable, "changed.log");
    assert!(changed.status.success(), "{}", support::messages(&changed));
    let updated = std::fs::read(&cache).expect("updated theme");
    assert_ne!(
        updated, previous,
        "editing a dependency must refresh the native artifact across restarts"
    );
    let gate = run_gate(&project, &executable, "valid-gate.log");
    assert!(
        gate.status.success() && support::messages(&gate).contains("Themosis imports: validated"),
        "{}",
        support::messages(&gate)
    );
    project.write("theme/tokens.json", r#"{"background":{"$type":"color","$value":{"colorSpace":"srgb","components":[0.8,0.2,0.3],"alpha":1}}}"#);
    let stale = run_gate(&project, &executable, "stale-gate.log");
    assert!(
        !stale.status.success(),
        "a valid source with a stale artifact must fail the gate"
    );
    project.write("theme/tokens.json", "{invalid");
    let failed = import(&project, &executable, "invalid.log");
    assert!(
        support::messages(&failed).contains("Error importing 'res://theme/probe.tms'"),
        "{}",
        support::messages(&failed)
    );
    let gate = run_gate(&project, &executable, "gate.log");
    assert!(
        !gate.status.success(),
        "invalid dependencies must fail export validation: {}",
        support::messages(&gate)
    );
    assert_eq!(
        std::fs::read(&cache).expect("last valid cache remains"),
        updated
    );
}

/// Editing a dependency inside a live editor session must reimport the theme
/// without dropping the plugin's own signal callbacks, and without leaving the
/// artifact stale.
#[test]
fn editor_sessions_reimport_changed_dependencies_without_losing_callbacks() {
    let Some(executable) = support::godot() else {
        return;
    };
    let Some(project) = ProbeProject::new() else {
        return;
    };
    project.write_theme_fixture();
    let imported = import(&project, &executable, "session-import.log");
    assert!(
        imported.status.success(),
        "{}",
        support::messages(&imported)
    );
    let manifest = project.imported_theme_text().expect("cached theme text");
    let first = fingerprint(&manifest).expect("manifest fingerprint");

    project.write("theme/tokens.json", r#"{"background":{"$type":"color","$value":{"colorSpace":"srgb","components":[0.9,0.2,0.3],"alpha":1}}}"#);
    let session = project.run_editor_session(&executable, 300, "session.log");
    support::assert_no_rust_failures(&session, &project.log("session.log"));

    let manifest = project.imported_theme_text().expect("refreshed theme text");
    let second = fingerprint(&manifest).expect("manifest fingerprint");
    assert_ne!(
        first, second,
        "a live editor session must reimport the theme after a dependency changed"
    );
}

#[test]
fn imported_artifacts_persist_their_dependency_manifest_and_fingerprint() {
    let Some(executable) = support::godot() else {
        return;
    };
    let Some(project) = ProbeProject::new() else {
        return;
    };
    project.write_theme_fixture();
    let imported = import(&project, &executable, "manifest.log");
    assert!(
        imported.status.success(),
        "{}",
        support::messages(&imported)
    );

    // The cache is text: it must name the theme, its whole dependency graph,
    // and the fingerprint the editor plugin and the gate compare against.
    let manifest = project.imported_theme_text().expect("cached theme text");
    assert!(
        manifest.contains(r#"resource_name = "probe""#),
        "imported themes keep their name:\n{manifest}"
    );
    assert!(
        manifest.contains("res://theme/probe.tms") && manifest.contains("res://theme/tokens.json"),
        "imported themes record every dependency:\n{manifest}"
    );
    let first = fingerprint(&manifest).expect("manifest fingerprint");

    project.write("theme/tokens.json", r#"{"background":{"$type":"color","$value":{"colorSpace":"srgb","components":[0.9,0.2,0.3],"alpha":1}}}"#);
    let refreshed = import(&project, &executable, "manifest-changed.log");
    assert!(
        refreshed.status.success(),
        "{}",
        support::messages(&refreshed)
    );
    let manifest = project.imported_theme_text().expect("cached theme text");
    let second = fingerprint(&manifest).expect("manifest fingerprint");
    assert_ne!(
        first, second,
        "editing a dependency must change the persisted fingerprint"
    );
}

/// A dependency two levels down must stale an import and fail the gate, not
/// only the resources a theme references directly.
#[test]
fn nested_resource_dependencies_invalidate_imports() {
    let Some(executable) = support::godot() else {
        return;
    };
    let Some(project) = ProbeProject::new() else {
        return;
    };
    project.write_theme_fixture();
    project.write(
        "theme/probe.tms",
        "theme Probe {\n    tokens \"tokens.json\"\n    style Card target=Button {\n        resource normal \"res://box.tres\"\n    }\n}\n",
    );
    project.write(
        "texture.tres",
        "[gd_resource type=\"GradientTexture2D\" format=3]\n\n[resource]\nwidth = 10\n",
    );
    project.write(
        "box.tres",
        "[gd_resource type=\"StyleBoxTexture\" load_steps=2 format=3]\n\n[ext_resource type=\"Texture2D\" path=\"res://texture.tres\" id=\"1\"]\n\n[resource]\ntexture = ExtResource(\"1\")\n",
    );

    let imported = import(&project, &executable, "nested-import.log");
    assert!(
        imported.status.success(),
        "{}",
        support::messages(&imported)
    );
    let manifest = project.imported_theme_text().expect("cached theme text");
    assert!(
        manifest.contains("res://box.tres") && manifest.contains("res://texture.tres"),
        "imported themes record their transitive dependencies:\n{manifest}"
    );
    let gate = run_gate(&project, &executable, "nested-valid-gate.log");
    assert!(gate.status.success(), "{}", support::messages(&gate));

    project.write(
        "texture.tres",
        "[gd_resource type=\"GradientTexture2D\" format=3]\n\n[resource]\nwidth = 20\n",
    );
    let stale = run_gate(&project, &executable, "nested-stale-gate.log");
    assert!(
        !stale.status.success(),
        "editing a nested dependency must fail the gate:\n{}",
        support::messages(&stale)
    );
}

/// Imports the project in an editor session.
fn import(project: &ProbeProject, executable: &str, log: &str) -> std::process::Output {
    project.run_import(executable, log)
}

/// Runs the native import validation gate.
fn run_gate(project: &ProbeProject, executable: &str, log: &str) -> std::process::Output {
    project.run_check_runner(executable, log)
}

/// Returns the fingerprint stored in an imported `.tres` manifest.
fn fingerprint(manifest: &str) -> Option<String> {
    manifest
        .lines()
        .find(|line| line.contains("_themosis_dependency_fingerprint"))
        .and_then(|line| line.split('=').nth(1))
        .map(|value| value.trim().trim_matches('"').to_owned())
}
