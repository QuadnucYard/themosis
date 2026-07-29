use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use themosis_compiler::{CompileErrors, compile_styles, resolve_tokens};
use themosis_core::{CompiledTheme, SourceId, StyleDocument, TokenDocument};
use thiserror::Error;

use crate::{InvalidSourcePath, SourceReadError, paths::normalize, provider::SourceProvider};

/// Compiles a root KDL document and its complete declared dependency tree.
pub fn compile_theme(
    provider: &impl SourceProvider,
    root: impl AsRef<Path>,
) -> Result<CompiledTheme, LoadError> {
    compile_theme_with_report(provider, root).into_result()
}

/// Compiles a source tree and reports every dependency discovered along the way.
///
/// Unlike [`compile_theme`], this preserves the dependency set when loading or
/// compilation fails. File-watching integrations can therefore retry when a
/// broken root or any dependency found before the failure changes.
pub fn compile_theme_with_report(
    provider: &impl SourceProvider,
    root: impl AsRef<Path>,
) -> CompilationReport {
    let root = match normalize(root.as_ref()) {
        Ok(root) => root,
        Err(source) => {
            return CompilationReport::new(
                BTreeSet::new(),
                Err(LoadError::InvalidPath {
                    owner: None,
                    path: root.as_ref().to_path_buf(),
                    source,
                }),
            );
        }
    };
    let mut loader = Loader::new(provider);
    if let Err(error) = loader.load_style(root.clone()) {
        return CompilationReport::new(loader.dependencies, Err(error));
    }
    let dependencies = loader.dependencies;
    let source_names = loader
        .source_paths
        .iter()
        .map(|(source, path)| (*source, path.display().to_string()))
        .collect::<BTreeMap<_, _>>();
    let token_documents = loader.tokens.into_values().collect::<Vec<_>>();
    let root_document = loader
        .styles
        .remove(&root)
        .expect("a successfully loaded root document is retained by the loader");
    let style_documents = std::iter::once(root_document)
        .chain(loader.styles.into_values())
        .collect::<Vec<_>>();
    let result = resolve_tokens(&token_documents)
        .map_err(|source| LoadError::Compile {
            source,
            sources: source_names.clone(),
        })
        .and_then(|tokens| {
            compile_styles(&style_documents, tokens).map_err(|source| LoadError::Compile {
                source,
                sources: source_names,
            })
        });
    CompilationReport::new(dependencies, result)
}

/// Result and source dependencies from one end-to-end compilation attempt.
#[derive(Debug)]
pub struct CompilationReport {
    dependencies: BTreeSet<PathBuf>,
    result: Result<CompiledTheme, LoadError>,
}

impl CompilationReport {
    fn new(dependencies: BTreeSet<PathBuf>, result: Result<CompiledTheme, LoadError>) -> Self {
        Self {
            dependencies,
            result,
        }
    }

    /// Returns normalized root-relative paths in deterministic order.
    #[must_use]
    pub const fn dependencies(&self) -> &BTreeSet<PathBuf> {
        &self.dependencies
    }

    /// Borrows the compiled theme or loading/compilation failure.
    pub const fn result(&self) -> Result<&CompiledTheme, &LoadError> {
        self.result.as_ref()
    }

    /// Consumes the report and returns the ordinary compilation result.
    pub fn into_result(self) -> Result<CompiledTheme, LoadError> {
        self.result
    }
}

struct Loader<'a, P> {
    provider: &'a P,
    next_source: u32,
    styles: BTreeMap<PathBuf, StyleDocument>,
    tokens: BTreeMap<PathBuf, TokenDocument>,
    source_paths: BTreeMap<SourceId, PathBuf>,
    visiting: Vec<PathBuf>,
    dependencies: BTreeSet<PathBuf>,
}

impl<'a, P: SourceProvider> Loader<'a, P> {
    fn new(provider: &'a P) -> Self {
        Self {
            provider,
            next_source: 0,
            styles: BTreeMap::new(),
            tokens: BTreeMap::new(),
            source_paths: BTreeMap::new(),
            visiting: Vec::new(),
            dependencies: BTreeSet::new(),
        }
    }

    fn source_id(&mut self, path: &Path) -> Result<SourceId, LoadError> {
        let current = self.next_source;
        self.next_source = current.checked_add(1).ok_or(LoadError::TooManySources)?;
        let source = SourceId::new(current);
        self.source_paths.insert(source, path.to_path_buf());
        Ok(source)
    }

    fn load_style(&mut self, path: PathBuf) -> Result<(), LoadError> {
        self.dependencies.insert(path.clone());
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
        let source_id = self.source_id(&path)?;
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
        self.dependencies.insert(path.clone());
        if self.tokens.contains_key(&path) {
            return Ok(());
        }
        let source = self.read(&path)?;
        let source_id = self.source_id(&path)?;
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
    #[error("theme compilation failed:\n{}", format_compile_failure(.source, .sources))]
    Compile {
        /// Semantic diagnostics.
        #[source]
        source: CompileErrors,
        /// Root-relative source names keyed by compiler identity.
        sources: BTreeMap<SourceId, String>,
    },
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

fn format_compile_failure(source: &CompileErrors, sources: &BTreeMap<SourceId, String>) -> String {
    source.render_with_source_names(sources)
}
