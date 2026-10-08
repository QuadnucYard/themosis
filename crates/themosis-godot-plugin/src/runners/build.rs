//! Native build runner materializing profile outputs.
//!
//! ```sh
//! godot --headless --path PROJECT --main-loop ThemosisBuildRunner -- --all
//! ```
//!
//! The runner materializes profiles through the same native builder the editor
//! uses, so headless output and editor output share one implementation.

// `#[class(init)]` expands to code that trips these lints on the `base` field.
#![allow(clippy::absolute_paths, clippy::redundant_field_names)]

use godot::{classes::Os, obj::Singleton, prelude::*};
use themosis_godot::reports::BatchOutcome;

use crate::{native::builder, project::config};

/// Godot main loop that builds profile outputs and exits.
#[derive(GodotClass)]
#[class(init, base=SceneTree)]
pub struct ThemosisBuildRunner {
    base: Base<SceneTree>,
}

#[godot_api]
impl ISceneTree for ThemosisBuildRunner {
    fn initialize(&mut self) {
        let code = run_build();
        self.base_mut().quit_ex().exit_code(code).done();
    }
}

enum Selection {
    All,
    Profile(String),
}

/// Runs the requested profile build and returns the process exit code.
fn run_build() -> i32 {
    let arguments = Os::singleton().get_cmdline_user_args();
    let selection = match parse_selection(&arguments) {
        Ok(selection) => selection,
        Err(error) => {
            eprintln!("Themosis: {error}");
            print_usage();
            return 2;
        }
    };
    let loaded = match config::load_config(true) {
        Ok(loaded) => loaded,
        Err(failure) => {
            eprintln!("Themosis: {}", failure.message);
            return 1;
        }
    };
    if !loaded.configured {
        eprintln!("Themosis: no profiles are configured in {}", loaded.path);
        return 1;
    }
    let batch = match selection {
        Selection::All => builder::build_all(&loaded.config),
        Selection::Profile(profile) => {
            BatchOutcome::from_results(vec![builder::build_named(&loaded.config, &profile)])
        }
    };
    for result in &batch.results {
        if result.ok() {
            println!(
                "Themosis[{}]: compiled {} -> {}",
                result.profile, result.source, result.output
            );
        } else {
            eprintln!("Themosis[{}]: {}", result.profile, result.error);
        }
    }
    for output in batch.outputs() {
        println!("Themosis: generated {output}");
    }
    if batch.ok() { 0 } else { 1 }
}

fn parse_selection(arguments: &PackedStringArray) -> Result<Selection, String> {
    let arguments = arguments
        .to_vec()
        .into_iter()
        .map(|argument| argument.to_string())
        .collect::<Vec<_>>();
    if arguments.len() == 1 && arguments[0] == "--all" {
        return Ok(Selection::All);
    }
    if arguments.len() == 2 && arguments[0] == "--profile" && !arguments[1].is_empty() {
        return Ok(Selection::Profile(arguments[1].clone()));
    }
    Err("select exactly one profile with --profile NAME or use --all".to_owned())
}

fn print_usage() {
    eprintln!("usage: godot --headless --path . --main-loop ThemosisBuildRunner -- --profile NAME");
    eprintln!("   or: godot --headless --path . --main-loop ThemosisBuildRunner -- --all");
}
