use std::path::{Path, PathBuf};

use themosis::{FileSystemSourceProvider, compile_theme};
use themosis_core::CompiledTheme;

/// Compiles a theme source tree from a root KDL source file.
pub(crate) fn compile_source(root: &Path) -> Result<CompiledTheme, String> {
    let canonical = root
        .canonicalize()
        .map_err(|error| format!("cannot open '{}': {error}", root.display()))?;
    let theme_root = canonical.parent().ok_or_else(|| {
        format!(
            "'{}' has no containing theme directory",
            canonical.display()
        )
    })?;
    let file_name = canonical
        .file_name()
        .ok_or_else(|| format!("'{}' is not a source file", canonical.display()))?;
    let provider = FileSystemSourceProvider::new(theme_root)
        .map_err(|error| format!("cannot open theme root '{}': {error}", theme_root.display()))?;
    compile_theme(&provider, PathBuf::from(file_name)).map_err(|error| error.to_string())
}
