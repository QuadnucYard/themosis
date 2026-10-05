//! Implementation of the `check` command.

use std::{path::PathBuf, process::ExitCode};

use clap::Args;

use crate::source::compile_source;

const FAILURE: u8 = 1;

/// Validates a theme source tree.
#[derive(Debug, Args)]
pub(crate) struct Check {
    /// Root style source file for the theme.
    #[arg(value_name = "ROOT")]
    root: PathBuf,
}

impl Check {
    /// Runs the command.
    pub(crate) fn run(self) -> ExitCode {
        match compile_source(&self.root) {
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
