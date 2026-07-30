//! Headless loading test for the GDExtension entry point.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use tempfile::TempDir;

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
