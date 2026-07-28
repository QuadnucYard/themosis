use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use thiserror::Error;

use crate::{InvalidSourcePath, paths::normalize};

/// Reads UTF-8 theme sources from a root-relative path namespace.
pub trait SourceProvider {
    /// Reads one normalized, relative source path.
    fn read(&self, path: &Path) -> Result<String, SourceReadError>;
}

/// Filesystem sources confined to a canonical theme root.
#[derive(Clone, Debug)]
pub struct FileSystemSourceProvider {
    root: PathBuf,
}

impl FileSystemSourceProvider {
    /// Opens a theme root and resolves it to a canonical path.
    pub fn new(root: impl AsRef<Path>) -> io::Result<Self> {
        Ok(Self {
            root: fs::canonicalize(root)?,
        })
    }

    /// Returns the canonical theme root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }
}

impl SourceProvider for FileSystemSourceProvider {
    fn read(&self, path: &Path) -> Result<String, SourceReadError> {
        let candidate = self.root.join(path);
        let canonical = fs::canonicalize(&candidate).map_err(|error| {
            SourceReadError::new(format!("cannot open '{}': {error}", candidate.display()))
        })?;
        if !canonical.starts_with(&self.root) {
            return Err(SourceReadError::new(format!(
                "source '{}' resolves outside theme root '{}'",
                path.display(),
                self.root.display()
            )));
        }
        fs::read_to_string(&canonical).map_err(|error| {
            SourceReadError::new(format!("cannot read '{}': {error}", canonical.display()))
        })
    }
}

/// In-memory sources using the same root-relative path namespace as the filesystem provider.
#[derive(Clone, Debug, Default)]
pub struct MemorySourceProvider {
    sources: BTreeMap<PathBuf, String>,
}

impl MemorySourceProvider {
    /// Creates an empty provider.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sources: BTreeMap::new(),
        }
    }

    /// Adds or replaces one source after validating its root-relative path.
    pub fn insert(
        &mut self,
        path: impl AsRef<Path>,
        source: impl Into<String>,
    ) -> Result<Option<String>, InvalidSourcePath> {
        let path = normalize(path.as_ref())?;
        Ok(self.sources.insert(path, source.into()))
    }
}

impl SourceProvider for MemorySourceProvider {
    fn read(&self, path: &Path) -> Result<String, SourceReadError> {
        self.sources.get(path).cloned().ok_or_else(|| {
            SourceReadError::new(format!("source '{}' does not exist", path.display()))
        })
    }
}

/// Provider-independent source read failure.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
#[error("{message}")]
pub struct SourceReadError {
    message: String,
}

impl SourceReadError {
    /// Creates a read failure with a user-facing explanation.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}
