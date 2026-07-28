use std::path::{Component, Path, PathBuf};

use thiserror::Error;

pub(crate) fn normalize(path: &Path) -> Result<PathBuf, InvalidSourcePath> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(segment) => normalized.push(segment),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(InvalidSourcePath::EscapesRoot);
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(InvalidSourcePath::Absolute);
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err(InvalidSourcePath::Empty);
    }
    Ok(normalized)
}

/// Reason a source path cannot be used in the root-relative namespace.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum InvalidSourcePath {
    /// No filename remains after normalization.
    #[error("source path is empty")]
    Empty,
    /// Absolute paths are not portable within a theme root.
    #[error("source path must be relative to the theme root")]
    Absolute,
    /// A parent component traverses above the theme root.
    #[error("source path escapes the theme root")]
    EscapesRoot,
}
