//! Native import gate validating discovery, compilation, and import freshness.
//!
//! ```sh
//! godot --headless --path PROJECT --main-loop ThemosisCheckRunner
//! ```
//!
//! The gate exits nonzero when a discovered root fails to compile or when its
//! imported resource is missing or stale, so export pipelines can stop before
//! shipping an outdated theme.

// `#[class(init)]` expands to code that trips these lints on the `base` field.
#![allow(clippy::absolute_paths, clippy::redundant_field_names)]

use godot::prelude::*;

use crate::{
    native::{
        generation::generate_from_project_path,
        import_cache::{
            DEPENDENCY_FINGERPRINT_META, dependency_fingerprint, load_imported_theme, meta_string,
        },
    },
    project::sources,
};

/// Godot main loop that validates imported themes and exits.
#[derive(GodotClass)]
#[class(init, base=SceneTree)]
pub struct ThemosisCheckRunner {
    base: Base<SceneTree>,
}

#[godot_api]
impl ISceneTree for ThemosisCheckRunner {
    fn initialize(&mut self) {
        let code = run_check();
        self.base_mut().quit_ex().exit_code(code).done();
    }
}

/// Validates discovery, compilation, and import freshness; returns the exit
/// code.
fn run_check() -> i32 {
    let mut succeeded = true;
    for source in sources::discover_roots() {
        let attempt = generate_from_project_path(&source);
        if let Err(failure) = &attempt.result {
            eprintln!("Themosis: {source}: {}", failure.message);
            succeeded = false;
            continue;
        }
        let imported = load_imported_theme(&source);
        let fingerprint = dependency_fingerprint(&attempt.dependency_list());
        let current = imported
            .as_ref()
            .map(|theme| meta_string(theme, DEPENDENCY_FINGERPRINT_META))
            .unwrap_or_default();
        if imported.is_none() || current != fingerprint {
            eprintln!("Themosis: {source} has no current import; run --editor --import first");
            succeeded = false;
        }
    }
    if succeeded {
        println!("Themosis imports: validated");
    }
    if succeeded { 0 } else { 1 }
}
