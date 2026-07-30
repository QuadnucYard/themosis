//! Godot target support for the `build` command.

use std::{path::Path, process::ExitCode};

use themosis_core::CompiledTheme;

use super::RuntimeOptions;

const FAILURE: u8 = 1;

pub(crate) fn run(
    root: &Path,
    output: &Path,
    theme: &CompiledTheme,
    runtime: &RuntimeOptions,
) -> ExitCode {
    let plan = match themosis_godot::plan_theme(theme) {
        Ok(plan) => plan,
        Err(error) => {
            eprintln!("themosis: Godot preparation failed:\n{error}");
            return ExitCode::from(FAILURE);
        }
    };
    let version = match runtime.build(root, output, &plan) {
        Ok(version) => version,
        Err(error) => {
            eprintln!("themosis: Godot generation failed:\n{error}");
            return ExitCode::from(FAILURE);
        }
    };
    println!(
        "generated Godot theme '{theme}' at '{output}' with {version}",
        theme = theme.name(),
        output = output.display(),
    );
    ExitCode::SUCCESS
}
