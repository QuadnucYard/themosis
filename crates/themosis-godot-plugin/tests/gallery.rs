//! Demo smoke tests for the checked-in component gallery.
//!
//! Unlike the probe suites, these exercise the developer workflow on a
//! throwaway copy of `examples/godot`: import the gallery, open its editor, and
//! drive the demo scene with its GDScript switcher. The copy loads the pinned
//! extension library, and every assertion requires observable output of that
//! library — an import that skipped it cannot pass.

mod support;

use support::GalleryProject;

/// The gallery imports and its demo control switches to the imported themes.
#[test]
fn gallery_scene_runs_without_script_errors() {
    let Some(executable) = support::godot() else {
        return;
    };
    let Some(gallery) = GalleryProject::new() else {
        return;
    };

    let imported = gallery.import(&executable);
    let messages = support::messages(&imported);
    assert!(
        imported.status.success()
            && !messages.contains("SCRIPT ERROR")
            && !messages.contains("Error importing 'res://theme"),
        "Godot could not import the gallery themes:\n{messages}"
    );
    let themes = gallery.imported_themes();
    assert!(
        themes.iter().any(|name| name.starts_with("light.tms"))
            && themes.iter().any(|name| name.starts_with("dark.tms")),
        "the pinned library did not import the gallery themes: {themes:?}\n{messages}"
    );

    let scene = gallery.run_scene(&executable);
    let messages = support::messages(&scene);
    assert!(
        scene.status.success()
            && !messages.contains("SCRIPT ERROR")
            && !messages.contains("Failed to load script"),
        "the demo scene failed to run:\n{messages}"
    );

    gallery.write("gallery_check.gd", include_str!("godot/gallery_check.gd"));
    let check = gallery.run_script(&executable, "res://gallery_check.gd", "gallery-check.log");
    let messages = support::messages(&check);
    assert!(
        check.status.success() && messages.contains("Gallery: passed"),
        "the demo control failed to switch themes:\n{messages}"
    );
}

/// The editor loads the auto-registered plugin and its dock.
#[test]
fn gallery_editor_loads_the_theme_dock() {
    let Some(executable) = support::godot() else {
        return;
    };
    let Some(gallery) = GalleryProject::new() else {
        return;
    };

    let imported = gallery.import(&executable);
    let messages = support::messages(&imported);
    assert!(
        imported.status.success() && !messages.contains("SCRIPT ERROR"),
        "Godot could not import the gallery:\n{messages}"
    );

    let editor = gallery.run_editor(&executable);
    let messages = support::messages(&editor);
    assert!(
        editor.status.success()
            && !messages.contains("SCRIPT ERROR")
            && !messages.contains("Failed to load script"),
        "Godot editor could not load the Themosis dock:\n{messages}"
    );

    gallery.write(
        "gallery_classes.gd",
        include_str!("godot/gallery_classes.gd"),
    );
    let classes = gallery.run_script(
        &executable,
        "res://gallery_classes.gd",
        "gallery-classes.log",
    );
    let messages = support::messages(&classes);
    assert!(
        classes.status.success() && messages.contains("Gallery classes: passed"),
        "the pinned library did not register the editor classes:\n{messages}"
    );
}
