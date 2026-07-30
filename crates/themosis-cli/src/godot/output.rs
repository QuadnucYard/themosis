//! Godot resource output path validation and localization.

use std::{
    env, fs,
    path::{Component, Path, PathBuf},
};

pub(super) fn localize_output(project: &Path, output: &Path) -> Result<String, String> {
    if output.extension().and_then(|value| value.to_str()) != Some("tres") {
        return Err(format!(
            "Godot theme output '{}' must use the .tres extension",
            output.display()
        ));
    }
    let text = output.to_string_lossy();
    if let Some(relative) = text.strip_prefix("res://") {
        let relative = safe_relative_path(Path::new(relative))?;
        let absolute = resolve_output_location(project, &project.join(relative))?;
        let relative = absolute
            .strip_prefix(project)
            .expect("validated output location is inside the project");
        return Ok(format!("res://{}", slash_path(relative)?));
    }
    let unresolved = if output.is_absolute() {
        output.to_path_buf()
    } else {
        env::current_dir()
            .map_err(|error| format!("cannot resolve current directory: {error}"))?
            .join(output)
    };
    let absolute = normalize_absolute_path(&unresolved)?;
    let absolute = resolve_output_location(project, &absolute)?;
    let relative = absolute
        .strip_prefix(project)
        .expect("validated output location is inside the project");
    Ok(format!("res://{}", slash_path(relative)?))
}

fn normalize_absolute_path(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!("output path '{}' is not absolute", path.display()));
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Path::new(std::path::MAIN_SEPARATOR_STR)),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(format!(
                        "output path '{}' escapes the filesystem root",
                        path.display()
                    ));
                }
            }
            Component::Normal(component) => normalized.push(component),
        }
    }
    Ok(normalized)
}

fn resolve_output_location(project: &Path, output: &Path) -> Result<PathBuf, String> {
    let mut ancestor = output.parent().ok_or_else(|| {
        format!(
            "Godot theme output '{}' has no containing directory",
            output.display()
        )
    })?;
    loop {
        match fs::symlink_metadata(ancestor) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                ancestor = ancestor.parent().ok_or_else(|| {
                    format!(
                        "cannot find an existing ancestor for output '{}'",
                        output.display()
                    )
                })?;
            }
            Err(error) => {
                return Err(format!(
                    "cannot inspect output ancestor '{}': {error}",
                    ancestor.display()
                ));
            }
        }
    }
    let canonical = ancestor.canonicalize().map_err(|error| {
        format!(
            "cannot resolve output ancestor '{}': {error}",
            ancestor.display()
        )
    })?;
    if !canonical.is_dir() {
        return Err(format!(
            "Godot output ancestor '{}' is not a directory",
            ancestor.display()
        ));
    }
    if !canonical.starts_with(project) {
        return Err(format!(
            "Godot theme output '{}' escapes project '{}' through '{}'",
            output.display(),
            project.display(),
            ancestor.display()
        ));
    }
    let suffix = output
        .strip_prefix(ancestor)
        .expect("ancestor was selected from the output path");
    Ok(canonical.join(suffix))
}

fn safe_relative_path(path: &Path) -> Result<PathBuf, String> {
    let mut safe = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(component) => safe.push(component),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!(
                    "Godot res:// output '{}' must stay inside the project",
                    path.display()
                ));
            }
        }
    }
    if safe.as_os_str().is_empty() {
        return Err("Godot theme output must name a file below res://".to_owned());
    }
    Ok(safe)
}

fn slash_path(path: &Path) -> Result<String, String> {
    path.components()
        .map(|component| match component {
            Component::Normal(component) => component
                .to_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("Godot resource path '{}' is not UTF-8", path.display())),
            _ => Err(format!(
                "Godot resource path '{}' is not project-relative",
                path.display()
            )),
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|components| components.join("/"))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{localize_output, safe_relative_path};

    #[test]
    fn rejects_resource_paths_that_escape_the_project() {
        assert!(safe_relative_path(Path::new("theme/generated.tres")).is_ok());
        assert!(safe_relative_path(Path::new("../generated.tres")).is_err());
    }

    #[test]
    fn accepts_localized_output_paths() {
        let project = tempfile::tempdir().expect("project directory is created");
        let project = project
            .path()
            .canonicalize()
            .expect("project directory is canonicalized");
        assert_eq!(
            localize_output(&project, Path::new("res://theme/generated.tres"))
                .expect("res path is valid"),
            "res://theme/generated.tres"
        );
    }

    #[test]
    fn rejects_outside_outputs_without_creating_directories() {
        let project = tempfile::tempdir().expect("project directory is created");
        let project = project
            .path()
            .canonicalize()
            .expect("project directory is canonicalized");
        let outside = tempfile::tempdir().expect("outside directory is created");
        let output = outside.path().join("new/directory/theme.tres");

        assert!(localize_output(&project, &output).is_err());
        assert!(!outside.path().join("new").exists());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_output_parent_symlinks_that_escape_the_project() {
        use std::os::unix::fs::symlink;

        let project = tempfile::tempdir().expect("project directory is created");
        let outside = tempfile::tempdir().expect("outside directory is created");
        symlink(
            outside.path().join("missing"),
            project.path().join("linked"),
        )
        .expect("dangling escape symlink is created");
        let project = project
            .path()
            .canonicalize()
            .expect("project directory is canonicalized");

        assert!(localize_output(&project, Path::new("res://linked/theme.tres")).is_err());
    }
}
