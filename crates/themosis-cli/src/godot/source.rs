//! Theme source path validation and localization.

use std::{env, path::Path};

use super::output::{safe_relative_path, slash_path};

/// Resolves a theme root to a project-confined `res://` path.
///
/// Both filesystem paths and `res://` paths are canonicalized, so a symlink
/// that escapes `project` is rejected before Godot starts. `res://` sources
/// still reject confined relative segments such as `..` textually.
pub(super) fn localize_source(project: &Path, root: &Path) -> Result<String, String> {
    let text = root.to_string_lossy();
    let candidate = if let Some(relative) = text.strip_prefix("res://") {
        let relative = safe_relative_path(Path::new(relative))?;
        project.join(relative)
    } else if root.is_absolute() {
        root.to_path_buf()
    } else {
        env::current_dir()
            .map_err(|error| format!("cannot resolve current directory: {error}"))?
            .join(root)
    };
    let absolute = candidate
        .canonicalize()
        .map_err(|error| format!("cannot open '{}': {error}", root.display()))?;
    if !absolute.is_file() {
        return Err(format!("theme source '{}' is not a file", root.display()));
    }
    if !absolute.starts_with(project) {
        return Err(format!(
            "theme source '{}' is outside Godot project '{}'",
            root.display(),
            project.display()
        ));
    }
    let relative = absolute
        .strip_prefix(project)
        .expect("canonical source is inside the project");
    Ok(format!("res://{}", slash_path(relative)?))
}
