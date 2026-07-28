use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use themosis_compiler::{CompileErrors, compile_styles, resolve_tokens};
use themosis_core::{CompiledTheme, SourceId, StyleDocument, TokenDocument};
use thiserror::Error;

use crate::{InvalidSourcePath, SourceProvider, SourceReadError, paths::normalize};

/// Compiles a root KDL document and its complete declared dependency tree.
pub fn compile_theme(
    provider: &impl SourceProvider,
    root: impl AsRef<Path>,
) -> Result<CompiledTheme, LoadError> {
    let root = normalize(root.as_ref()).map_err(|source| LoadError::InvalidPath {
        owner: None,
        path: root.as_ref().to_path_buf(),
        source,
    })?;
    let mut loader = Loader::new(provider);
    loader.load_style(root)?;
    let tokens = resolve_tokens(&loader.tokens.into_values().collect::<Vec<_>>())
        .map_err(LoadError::Compile)?;
    compile_styles(&loader.styles.into_values().collect::<Vec<_>>(), tokens)
        .map_err(LoadError::Compile)
}

struct Loader<'a, P> {
    provider: &'a P,
    next_source: u32,
    styles: BTreeMap<PathBuf, StyleDocument>,
    tokens: BTreeMap<PathBuf, TokenDocument>,
    visiting: Vec<PathBuf>,
}

impl<'a, P: SourceProvider> Loader<'a, P> {
    fn new(provider: &'a P) -> Self {
        Self {
            provider,
            next_source: 0,
            styles: BTreeMap::new(),
            tokens: BTreeMap::new(),
            visiting: Vec::new(),
        }
    }

    fn source_id(&mut self) -> Result<SourceId, LoadError> {
        let current = self.next_source;
        self.next_source = current.checked_add(1).ok_or(LoadError::TooManySources)?;
        Ok(SourceId::new(current))
    }

    fn load_style(&mut self, path: PathBuf) -> Result<(), LoadError> {
        if self.styles.contains_key(&path) {
            return Ok(());
        }
        if let Some(index) = self
            .visiting
            .iter()
            .position(|candidate| candidate == &path)
        {
            let mut cycle = self.visiting[index..].to_vec();
            cycle.push(path);
            return Err(LoadError::ImportCycle { cycle });
        }

        self.visiting.push(path.clone());
        let source = self.read(&path)?;
        let source_id = self.source_id()?;
        let document =
            themosis_kdl::parse(&path.to_string_lossy(), source_id, &source).map_err(|source| {
                LoadError::Kdl {
                    path: path.clone(),
                    source,
                }
            })?;

        for token in document.token_sources() {
            let token_path = resolve_reference(&path, token)?;
            self.load_tokens(token_path)?;
        }
        for import in document.imports() {
            let import_path = resolve_reference(&path, import)?;
            self.load_style(import_path)?;
        }

        let popped = self.visiting.pop();
        debug_assert_eq!(popped.as_ref(), Some(&path));
        self.styles.insert(path, document);
        Ok(())
    }

    fn load_tokens(&mut self, path: PathBuf) -> Result<(), LoadError> {
        if self.tokens.contains_key(&path) {
            return Ok(());
        }
        let source = self.read(&path)?;
        let source_id = self.source_id()?;
        let document =
            themosis_tokens::parse(source_id, &source).map_err(|source| LoadError::Tokens {
                path: path.clone(),
                source,
            })?;
        self.tokens.insert(path, document);
        Ok(())
    }

    fn read(&self, path: &Path) -> Result<String, LoadError> {
        self.provider.read(path).map_err(|source| LoadError::Read {
            path: path.to_path_buf(),
            source,
        })
    }
}

fn resolve_reference(owner: &Path, reference: &str) -> Result<PathBuf, LoadError> {
    let joined = owner
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join(reference);
    normalize(&joined).map_err(|source| LoadError::InvalidPath {
        owner: Some(owner.to_path_buf()),
        path: PathBuf::from(reference),
        source,
    })
}

/// Failure while discovering, parsing, or compiling a theme source tree.
#[derive(Debug, Error)]
pub enum LoadError {
    /// A root or declared dependency path is invalid.
    #[error("{}", format_invalid_path(.owner, .path, .source))]
    InvalidPath {
        /// Declaring document, or `None` for the requested root.
        owner: Option<PathBuf>,
        /// Invalid path as written.
        path: PathBuf,
        /// Path policy failure.
        #[source]
        source: InvalidSourcePath,
    },
    /// A source could not be read.
    #[error("failed to read '{path}': {source}")]
    Read {
        /// Requested root-relative path.
        path: PathBuf,
        /// Provider failure.
        #[source]
        source: SourceReadError,
    },
    /// A KDL source is malformed.
    #[error("failed to parse '{path}': {source}")]
    Kdl {
        /// Root-relative source path.
        path: PathBuf,
        /// Parser failure.
        #[source]
        source: themosis_kdl::ParseError,
    },
    /// A token JSON source is malformed.
    #[error("failed to parse '{path}': {source}")]
    Tokens {
        /// Root-relative source path.
        path: PathBuf,
        /// Parser failures.
        #[source]
        source: themosis_tokens::ParseErrors,
    },
    /// KDL imports contain a cycle.
    #[error("style import cycle: {}", format_path_cycle(.cycle))]
    ImportCycle {
        /// Closed path through the cycle, with the first path repeated last.
        cycle: Vec<PathBuf>,
    },
    /// More sources were loaded than can be identified.
    #[error("theme contains too many sources")]
    TooManySources,
    /// Parsed documents failed semantic compilation.
    #[error("theme compilation failed: {0}")]
    Compile(CompileErrors),
}

fn format_invalid_path(owner: &Option<PathBuf>, path: &Path, source: &InvalidSourcePath) -> String {
    match owner {
        Some(owner) => format!(
            "invalid path '{}' declared by '{}': {source}",
            path.display(),
            owner.display()
        ),
        None => format!("invalid root path '{}': {source}", path.display()),
    }
}

fn format_path_cycle(cycle: &[PathBuf]) -> String {
    cycle
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(" -> ")
}
