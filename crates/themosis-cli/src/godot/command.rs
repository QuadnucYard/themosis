//! `themosis godot` subcommands.

use std::{path::PathBuf, process::ExitCode};

use clap::{Args, Subcommand};

use super::runtime::RuntimeOptions;

const FAILURE: u8 = 1;

/// Project-aware Godot validation and generation.
#[derive(Debug, Args)]
pub(crate) struct Godot {
    #[command(subcommand)]
    action: GodotAction,
}

#[derive(Debug, Subcommand)]
enum GodotAction {
    /// Compile and map sources against a live Godot engine.
    Check(GodotCheck),
    /// Compile sources and write a native Godot theme.
    Build(GodotBuild),
}

#[derive(Debug, Args)]
struct GodotCheck {
    #[command(flatten)]
    runtime: RuntimeOptions,
    /// Root style source file for the theme.
    #[arg(value_name = "ROOT")]
    root: PathBuf,
}

#[derive(Debug, Args)]
struct GodotBuild {
    #[command(flatten)]
    runtime: RuntimeOptions,
    /// Generated theme resource path.
    #[arg(short, long, value_name = "FILE")]
    output: PathBuf,
    /// Root style source file for the theme.
    #[arg(value_name = "ROOT")]
    root: PathBuf,
}

impl Godot {
    /// Runs the selected subcommand.
    pub(crate) fn run(self) -> ExitCode {
        match self.action {
            GodotAction::Check(command) => command.run(),
            GodotAction::Build(command) => command.run(),
        }
    }
}

impl GodotCheck {
    fn run(self) -> ExitCode {
        match self.runtime.check(&self.root) {
            Ok(version) => {
                println!(
                    "Godot mappings for '{}' validate successfully with {version}",
                    self.root.display(),
                );
                ExitCode::SUCCESS
            }
            Err(error) => report(error),
        }
    }
}

impl GodotBuild {
    fn run(self) -> ExitCode {
        match self.runtime.build(&self.root, &self.output) {
            Ok(version) => {
                println!(
                    "generated Godot theme at '{output}' with {version}",
                    output = self.output.display()
                );
                ExitCode::SUCCESS
            }
            Err(error) => report(error),
        }
    }
}

fn report(error: String) -> ExitCode {
    eprintln!("themosis: Godot operation failed:\n{error}");
    ExitCode::from(FAILURE)
}
