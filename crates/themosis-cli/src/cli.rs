//! Command-line argument parsing and command dispatch.

use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::commands::{build::Build, check::Check};

/// Themosis command-line interface.
#[derive(Debug, Parser)]
#[command(name = "themosis")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Generate a targeted backend artifact.
    Build(Build),
    /// Validate a theme source tree.
    Check(Check),
}

/// Parses the command line and runs the selected command.
pub(crate) fn run() -> ExitCode {
    match Cli::try_parse() {
        Ok(cli) => match cli.command {
            Command::Build(command) => command.run(),
            Command::Check(command) => command.run(),
        },
        Err(error) => error.exit(),
    }
}
