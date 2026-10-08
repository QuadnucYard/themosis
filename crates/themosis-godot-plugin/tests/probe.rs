//! Extension loading and native mapping probes against a throwaway project.

mod support;

use support::ProbeProject;

/// The extension loads and every native mapping probe passes.
#[test]
fn native_mapping_probes_pass() {
    let Some(executable) = support::godot() else {
        return;
    };
    let Some(_library) = support::probe_library() else {
        return;
    };
    let Some(project) = ProbeProject::new() else {
        return;
    };
    project.write("test.gd", include_str!("godot/test.gd"));

    let output = project.run_script(&executable, "res://test.gd", "native-mappings.log");
    let messages = support::messages(&output);
    assert!(
        output.status.success() && !messages.contains("SCRIPT ERROR"),
        "Godot failed to load the extension or a native mapping probe failed:\n{messages}"
    );
}
