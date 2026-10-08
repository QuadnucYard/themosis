//! Rebuilds must read edited resources without mutating the last valid theme.

mod support;

use support::ProbeProject;

#[test]
fn rebuild_reads_disk_and_preserves_the_previous_theme() {
    let Some(executable) = support::godot() else {
        return;
    };
    let Some(_library) = support::probe_library() else {
        return;
    };
    let Some(project) = ProbeProject::new() else {
        return;
    };
    project.write(
        "theme.tms",
        "theme Probe {\n    style Probe target=Button {\n        resource normal \"res://box.tres\"\n    }\n}\n",
    );
    project.write("test.gd", include_str!("godot/fresh_resources.gd"));

    let output = project.run_script(&executable, "res://test.gd", "fresh-resources.log");
    let messages = support::messages(&output);
    assert!(
        output.status.success()
            && messages.contains("Fresh resources: passed")
            && !messages.contains("SCRIPT ERROR"),
        "{messages}"
    );
}
