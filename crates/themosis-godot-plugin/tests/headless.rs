//! Headless loading test for the GDExtension entry point.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use tempfile::TempDir;

static EXAMPLE_PROJECT_LOCK: Mutex<()> = Mutex::new(());

fn godot() -> Option<String> {
    if let Some(executable) = std::env::var_os("THEMOSIS_GODOT_BINARY") {
        return Some(executable.to_string_lossy().into_owned());
    }
    for executable in ["godot", "godot4"] {
        if Command::new(executable).arg("--version").output().is_ok() {
            return Some(executable.to_owned());
        }
    }
    eprintln!("skipping headless smoke test: Godot is not installed");
    None
}

fn command(executable: &str, project: &Path, flags: &[&str]) -> Command {
    let mut command = Command::new(executable);
    command.args(flags).arg("--path").arg(project);
    command
}

struct TestProject {
    directory: PathBuf,
    logs: TempDir,
}

impl TestProject {
    fn new(directory: impl Into<PathBuf>) -> Self {
        let project = Self {
            directory: directory.into(),
            logs: tempfile::tempdir().expect("Godot test log directory is created"),
        };
        project.path();
        project
    }

    fn path(&self) -> &Path {
        let path = &self.directory;
        assert!(
            path.is_dir(),
            "Godot project directory '{}' is missing",
            path.display()
        );
        assert!(
            path.join("project.godot").is_file(),
            "Godot project directory '{}' has no project.godot",
            path.display()
        );
        path
    }

    fn log(&self, name: &str) -> PathBuf {
        self.logs.path().join(name)
    }
}

#[test]
fn godot_loads_the_extension_when_available() {
    let project = TestProject::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/godot"));
    let Some(executable) = godot() else {
        return;
    };

    let output = command(&executable, project.path(), &["--headless"])
        .arg("--log-file")
        .arg(project.log("backend.log"))
        .args(["--script", "res://test.gd"])
        .output()
        .expect("Godot starts");

    assert!(
        output.status.success(),
        "Godot failed to load the extension:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "not ready"]
fn theme_switcher_example_imports_and_switches_themes() {
    let Some(executable) = godot() else {
        return;
    };
    let _project_lock = EXAMPLE_PROJECT_LOCK.lock().expect("example lock is usable");
    let project =
        TestProject::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/godot"));

    let imported = command(
        &executable,
        project.path(),
        &["--headless", "--editor", "--import"],
    )
    .arg("--log-file")
    .arg(project.log("example-import.log"))
    .output()
    .expect("Godot project import starts");
    let output = command(&executable, project.path(), &["--headless"])
        .arg("--log-file")
        .arg(project.log("example-test.log"))
        .args(["--script", "res://smoke_test.gd"])
        .output()
        .expect("Godot starts");
    let import_messages = format!(
        "{}\n{}",
        String::from_utf8_lossy(&imported.stdout),
        String::from_utf8_lossy(&imported.stderr)
    );
    assert!(
        imported.status.success()
            && !import_messages.contains("SCRIPT ERROR")
            && !import_messages.contains("Error importing 'res://theme"),
        "Godot could not import the .tms themes:\n{import_messages}"
    );
    assert!(
        output.status.success(),
        "Godot theme-switcher smoke test failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "not ready"]
fn addon_profile_configuration_and_build_workflows() {
    let Some(executable) = godot() else {
        return;
    };
    let _project_lock = EXAMPLE_PROJECT_LOCK.lock().expect("example lock is usable");
    let project =
        TestProject::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/godot"));
    let output = command(&executable, project.path(), &["--headless"])
        .arg("--log-file")
        .arg(project.log("profile-test.log"))
        .args(["--script", "res://profile_test.gd"])
        .output()
        .expect("Godot profile tests start");

    assert!(
        output.status.success(),
        "Godot addon profile tests failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "not ready"]
fn godot_editor_loads_the_theme_dock() {
    let Some(executable) = godot() else {
        return;
    };
    let _project_lock = EXAMPLE_PROJECT_LOCK.lock().expect("example lock is usable");
    let project =
        TestProject::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/godot"));
    let output = command(&executable, project.path(), &["--headless", "--editor"])
        .arg("--log-file")
        .arg(project.log("editor-dock-test.log"))
        .args(["--quit-after", "3"])
        .output()
        .expect("Godot editor starts");
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(output.status.success(), "Godot editor failed:\n{combined}");
    assert!(
        !combined.contains("SCRIPT ERROR") && !combined.contains("Failed to load script"),
        "Godot editor could not load the Themosis dock:\n{combined}"
    );
}
