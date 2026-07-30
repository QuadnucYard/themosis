use std::{
    io::Read,
    path::{Component, Path},
};

use godot::{classes::file_access::ModeFlags, prelude::GFile};
use themosis::{SourceProvider, SourceReadError};

/// Reads theme sources through Godot's `res://` virtual filesystem.
///
/// Unlike an operating-system filesystem provider, this provider can read
/// project files stored in an exported PCK. Paths supplied by the facade are
/// normalized and relative to the Godot project root. It must be used while
/// Godot is initialized and on a thread allowed to call `FileAccess`.
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

    use super::resource_path;

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
}
