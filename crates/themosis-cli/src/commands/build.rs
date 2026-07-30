//! Implementation of the `build` command.

use std::{path::PathBuf, process::ExitCode};

use clap::Args;

use crate::{
    source::compile_source,
    target::{Target, TargetOptions},
};

const FAILURE: u8 = 1;

/// Builds one backend artifact from a theme source tree.
#[derive(Debug, Args)]
pub(crate) struct Build {
    /// Backend artifact format to generate.
    #[arg(long, value_enum)]
    target: Target,
    /// Generated backend artifact path.
    #[arg(short, long, value_name = "FILE")]
    output: PathBuf,
    #[command(flatten)]
    options: TargetOptions,
    /// Root style source file for the theme.
    #[arg(value_name = "ROOT")]
    root: PathBuf,
}

impl Build {
    /// Runs the command.
    pub(crate) fn run(self) -> ExitCode {
        let theme = match compile_source(&self.root) {
            Ok(theme) => theme,
            Err(error) => {
                eprintln!("themosis: {error}");
                return ExitCode::from(FAILURE);
            }
        };
        self.target
            .build(&self.root, &self.output, &theme, &self.options)
    }
}
