//! Confined native resource persistence shared by CLI and addon operations.

use std::{
    fs,
    path::{Path, PathBuf},
};

use godot::{
    classes::{ProjectSettings, ResourceSaver, Theme},
    global::Error as GodotError,
    obj::Singleton,
    prelude::*,
};
use themosis_godot::RunnerDiagnostic;

/// Serializes to a unique sibling and atomically replaces the destination.
/// On failure the old destination remains intact and the temporary is removed.
pub(crate) fn save_theme(theme: &Gd<Theme>, output: &str) -> Result<(), Box<RunnerDiagnostic>> {
    write_output(output, "tres", |temporary| {
        let error = ResourceSaver::singleton()
            .save_ex(theme)
            .path(temporary)
            .done();
        if error != GodotError::OK {
            return Err(failure(
                "save_failed",
                format!("could not serialize theme '{output}': {error:?}"),
            ));
        }
        Ok(())
    })
}

fn write_output(
    output: &str,
    extension: &str,
    write: impl FnOnce(&str) -> Result<(), Box<RunnerDiagnostic>>,
) -> Result<(), Box<RunnerDiagnostic>> {
    let settings = ProjectSettings::singleton();
    let project = PathBuf::from(settings.globalize_path("res://").to_string());
    let absolute = confined_output(&project, output, extension)
        .map_err(|error| Box::new(RunnerDiagnostic::new("invalid_output", error)))?;
    let directory = absolute
        .parent()
        .expect("output has a containing directory");
    fs::create_dir_all(directory).map_err(|error| failure("output_directory", error))?;
    // Closing the handle before ResourceSaver opens it also supports Windows.
    let temporary = tempfile::Builder::new()
        .prefix(".themosis-")
        .suffix(&format!(".{extension}"))
        .tempfile_in(directory)
        .map_err(|error| failure("save_failed", error))?
        .into_temp_path();
    let temporary_text = temporary
        .to_str()
        .ok_or_else(|| failure("invalid_output", "output path is not UTF-8"))?;
    write(temporary_text)?;
    temporary
        .persist(&absolute)
        .map_err(|error| failure("replace_failed", error.error))?;
    Ok(())
}

fn failure(code: &str, error: impl std::fmt::Display) -> Box<RunnerDiagnostic> {
    Box::new(RunnerDiagnostic::new(code, error.to_string()))
}

/// Validates confinement before creating any directories or following symlinks.
fn confined_output(project: &Path, output: &str, extension: &str) -> Result<PathBuf, String> {
    let relative = output
        .strip_prefix("res://")
        .filter(|relative| {
            valid_relative_path(relative) && relative.ends_with(&format!(".{extension}"))
        })
        .ok_or_else(|| format!("output must be a confined res:// path ending in .{extension}"))?;
    let project = project.canonicalize().map_err(|error| error.to_string())?;
    let candidate = project.join(relative);
    let mut ancestor = candidate
        .parent()
        .expect("validated output is below the project");
    loop {
        match fs::symlink_metadata(ancestor) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                ancestor = ancestor
                    .parent()
                    .ok_or_else(|| "output has no existing ancestor".to_owned())?;
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    let canonical = ancestor.canonicalize().map_err(|error| error.to_string())?;
    if !canonical.is_dir() || !canonical.starts_with(&project) {
        return Err(format!(
            "theme output '{output}' escapes the project or has an invalid parent"
        ));
    }
    Ok(canonical.join(
        candidate
            .strip_prefix(ancestor)
            .expect("ancestor is a candidate prefix"),
    ))
}

/// Returns whether a resource path is lexically confined to the project.
pub(crate) fn valid_relative_path(relative: &str) -> bool {
    !relative.is_empty()
        && !relative.contains('\\')
        && relative
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

#[cfg(test)]
mod tests {
    use super::confined_output;

    #[test]
    fn rejects_escapes_without_creating_directories() {
        let project = tempfile::tempdir().expect("project");
        assert!(confined_output(project.path(), "res://../outside/theme.tres", "tres").is_err());
        assert!(confined_output(project.path(), "res://new/theme.tres", "tres").is_ok());
        assert!(!project.path().join("new").exists());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_output_parents() {
        let project = tempfile::tempdir().expect("project");
        let outside = tempfile::tempdir().expect("outside");
        std::os::unix::fs::symlink(outside.path(), project.path().join("linked")).expect("symlink");
        assert!(confined_output(project.path(), "res://linked/theme.tres", "tres").is_err());
        assert!(!outside.path().join("theme.tres").exists());
    }
}
