//! Implementation of the `check` command.

use std::{path::PathBuf, process::ExitCode};

use clap::Args;
use themosis::{FileSystemSourceProvider, compile_theme};

const FAILURE: u8 = 1;

/// Validates a theme source tree.
#[derive(Debug, Args)]
pub(crate) struct Check {
    /// Root KDL source file for the theme.
    #[arg(value_name = "root.kdl")]
    root: PathBuf,
}

impl Check {
    /// Runs the command.
    pub(crate) fn run(self) -> ExitCode {
        let canonical = match self.root.canonicalize() {
            Ok(path) => path,
            Err(error) => {
                eprintln!("themosis: cannot open '{}': {error}", self.root.display());
                return ExitCode::from(FAILURE);
            }
        };
        let Some(theme_root) = canonical.parent() else {
            eprintln!(
                "themosis: '{}' has no containing theme directory",
                canonical.display()
            );
            return ExitCode::from(FAILURE);
        };
        let Some(file_name) = canonical.file_name() else {
            eprintln!("themosis: '{}' is not a source file", canonical.display());
            return ExitCode::from(FAILURE);
        };
        let provider = match FileSystemSourceProvider::new(theme_root) {
            Ok(provider) => provider,
            Err(error) => {
                eprintln!(
                    "themosis: cannot open theme root '{}': {error}",
                    theme_root.display()
                );
                return ExitCode::from(FAILURE);
            }
        };

        match compile_theme(&provider, PathBuf::from(file_name)) {
            Ok(theme) => {
                println!("theme '{}' sources compile successfully", theme.name());
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("themosis: {error}");
                ExitCode::from(FAILURE)
            }
        }
    }
}
