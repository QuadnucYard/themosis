use std::{
    io::Read,
    path::{Component, Path},
};

use godot::{
    classes::{ProjectSettings, file_access::ModeFlags},
    obj::Singleton,
    prelude::*,
};
use themosis::{SourceProvider, SourceReadError};

/// Reads theme sources through Godot's `res://` virtual filesystem.
///
/// Unlike an operating-system filesystem provider, this provider can read
/// project files stored in an exported PCK. Paths supplied by the facade are
/// normalized and relative to the Godot project root. It must be used while
/// Godot is initialized and on a thread allowed to call `FileAccess`.
///
/// Godot's `FileAccess` follows symlinks transparently, so a source that is a
/// link to a file outside the project would otherwise be importable. Every
/// read therefore validates the physical file against the canonical project
/// root first; symlinks that resolve inside the project remain readable.
#[derive(Clone, Copy, Debug, Default)]
pub struct GodotSourceProvider;

impl GodotSourceProvider {
    /// Creates a project-root source provider.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl SourceProvider for GodotSourceProvider {
    fn read(&self, path: &Path) -> Result<String, SourceReadError> {
        let resource_path = resource_path(path)?;
        confine_physical(&resource_path)?;
        let mut file = GFile::open(&resource_path, ModeFlags::READ).map_err(|error| {
            SourceReadError::new(format!("cannot open '{resource_path}': {error}"))
        })?;
        let mut source = String::new();
        file.read_to_string(&mut source).map_err(|error| {
            SourceReadError::new(format!("cannot read '{resource_path}' as UTF-8: {error}"))
        })?;
        Ok(source)
    }
}

/// Confines a `res://` source's physical file to the Godot project root.
fn confine_physical(resource_path: &str) -> Result<(), SourceReadError> {
    let settings = ProjectSettings::singleton();
    let project = settings.globalize_path("res://").to_string();
    let physical = settings.globalize_path(resource_path).to_string();
    validate_physical(Path::new(&project), Path::new(&physical))
}

/// Validates that a physical source file is stored inside the project.
///
/// A path without a physical file cannot escape the project: it is either
/// missing — the engine reports that itself — or stored in the project's PCK,
/// which only contains project assets. Existing files are resolved through
/// symlinks and compared against the canonical project root.
fn validate_physical(project: &Path, physical: &Path) -> Result<(), SourceReadError> {
    let canonical_project = project.canonicalize().map_err(|error| {
        SourceReadError::new(format!(
            "cannot resolve Godot project root '{}': {error}",
            project.display()
        ))
    })?;
    match physical.canonicalize() {
        Ok(canonical) if canonical.starts_with(&canonical_project) => Ok(()),
        Ok(canonical) => Err(SourceReadError::new(format!(
            "source '{}' resolves outside the Godot project ('{}')",
            physical.display(),
            canonical.display()
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(SourceReadError::new(format!(
            "cannot resolve source '{}': {error}",
            physical.display()
        ))),
    }
}

fn resource_path(path: &Path) -> Result<String, SourceReadError> {
    let mut segments = Vec::new();
    for component in path.components() {
        let Component::Normal(segment) = component else {
            return Err(SourceReadError::new(format!(
                "source path '{}' is not normalized relative to the project root",
                path.display()
            )));
        };
        let segment = segment.to_str().ok_or_else(|| {
            SourceReadError::new(format!("source path '{}' is not UTF-8", path.display()))
        })?;
        segments.push(segment);
    }
    if segments.is_empty() {
        return Err(SourceReadError::new("source path is empty"));
    }
    Ok(format!("res://{}", segments.join("/")))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{resource_path, validate_physical};

    #[test]
    fn creates_project_resource_paths() {
        assert_eq!(
            resource_path(Path::new("theme/styles/buttons.kdl"))
                .expect("path is relative")
                .to_string(),
            "res://theme/styles/buttons.kdl"
        );
        assert!(resource_path(Path::new("../outside.kdl")).is_err());
    }

    #[test]
    fn confines_physical_sources_to_the_project_root() {
        let project = tempfile::tempdir().expect("project");
        let inside = project.path().join("theme/fragment.kdl");
        std::fs::create_dir_all(inside.parent().expect("fragment parent"))
            .expect("theme directory");
        std::fs::write(&inside, "style Probe {}\n").expect("fragment");

        assert!(validate_physical(project.path(), &inside).is_ok());
        // Missing files are the engine's business: PCK-stored sources have no
        // physical counterpart.
        assert!(validate_physical(project.path(), &project.path().join("missing.kdl")).is_ok());

        let outside = tempfile::tempdir().expect("outside");
        let escaped = outside.path().join("fragment.kdl");
        std::fs::write(&escaped, "style Probe {}\n").expect("outside fragment");
        assert!(validate_physical(project.path(), &escaped).is_err());

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let escaping_link = project.path().join("linked.kdl");
            symlink(&escaped, &escaping_link).expect("escape symlink");
            assert!(validate_physical(project.path(), &escaping_link).is_err());

            let confined_link = project.path().join("confined.kdl");
            symlink(&inside, &confined_link).expect("confined symlink");
            assert!(validate_physical(project.path(), &confined_link).is_ok());
        }
    }
}
