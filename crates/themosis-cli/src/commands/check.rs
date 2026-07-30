//! Implementation of the `check` command.

use std::{path::PathBuf, process::ExitCode};

use clap::Args;

use crate::{
    source::compile_source,
    target::{Target, TargetOptions},
};

const FAILURE: u8 = 1;

/// Validates a theme source tree.
#[derive(Debug, Args)]
pub(crate) struct Check {
    /// Also validate constraints for a targeted backend.
    #[arg(long, value_enum)]
    target: Option<Target>,
    #[command(flatten)]
    options: TargetOptions,
    /// Root style source file for the theme.
    #[arg(value_name = "ROOT")]
    root: PathBuf,
}

impl Check {
    /// Runs the command.
    pub(crate) fn run(self) -> ExitCode {
        match compile_source(&self.root) {
            Ok(theme) => {
                if let Some(target) = self.target {
                    return target.check(&self.root, &theme, &self.options);
                }

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
