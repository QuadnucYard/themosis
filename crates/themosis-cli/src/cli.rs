//! Command-line argument parsing and command dispatch.

use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::commands::check::Check;
#[cfg(feature = "godot")]
use crate::godot::Godot;

/// Themosis command-line interface.
#[derive(Debug, Parser)]
#[command(name = "themosis")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Validate a theme source tree without a running engine.
    Check(Check),
    /// Validate or generate a Godot theme through a running engine.
    #[cfg(feature = "godot")]
    Godot(Godot),
}

/// Parses the command line and runs the selected command.
pub(crate) fn run() -> ExitCode {
    match Cli::try_parse() {
        Ok(cli) => run_command(cli.command),
        Err(error) => error.exit(),
    }
}

/// Runs a parsed subcommand and returns its exit code.
fn run_command(command: Command) -> ExitCode {
    match command {
        Command::Check(command) => command.run(),
        #[cfg(feature = "godot")]
        Command::Godot(command) => command.run(),
    }
}
