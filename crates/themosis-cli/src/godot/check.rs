//! Godot target support for the `check` command.

use std::{path::Path, process::ExitCode};

use themosis_core::CompiledTheme;

use super::RuntimeOptions;

const FAILURE: u8 = 1;

pub(crate) fn run(root: &Path, theme: &CompiledTheme, runtime: &RuntimeOptions) -> ExitCode {
    let plan = match themosis_godot::plan_theme(theme) {
        Ok(plan) => plan,
        Err(error) => {
            eprintln!("themosis: Godot preparation failed:\n{error}");
            return ExitCode::from(FAILURE);
        }
    };
    match runtime.check(root, &plan) {
        Ok(version) => {
            println!(
                "theme '{theme}' sources and Godot mappings validate successfully with {version}",
                theme = theme.name(),
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("themosis: Godot mapping failed:\n{error}");
            ExitCode::from(FAILURE)
        }
    }
}
