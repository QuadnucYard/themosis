//! Native CLI runner executed as Godot's main loop.
//!
//! `themosis-cli` starts a project that ships the Themosis addon with
//! `godot --headless --path PROJECT --main-loop ThemosisCliRunner --
//! REQUEST RESPONSE`. The runner reads `REQUEST`, compiles and materializes its
//! theme through the same native backend the editor importer uses, and writes
//! `RESPONSE`. No GDScript participates in mapping or process control.

use std::{fs, path::PathBuf};

use godot::{
    classes::{Engine, Os},
    obj::{Singleton, WithBaseField},
    prelude::*,
};
use themosis_godot::{
    GodotVersion, RUNNER_SCHEMA_VERSION, RunnerDiagnostic, RunnerOperation, RunnerRequest,
    RunnerResponse,
};

use crate::native::{
    generation::generate_from_project_path,
    materialize::{save_theme, valid_relative_path},
};

mod api {
    #![allow(clippy::redundant_field_names)]
    #![allow(clippy::absolute_paths)]

    use super::*;

    /// Godot main loop that performs one CLI request and exits.
    ///
    /// Godot instantiates this class for `--main-loop`; the two paths after `--` are
    /// the request and response files.
    #[derive(GodotClass)]
    #[class(init, base=SceneTree)]
    pub struct ThemosisCliRunner {
        base: Base<SceneTree>,
    }

    #[godot_api]
    impl ISceneTree for ThemosisCliRunner {
        fn initialize(&mut self) {
            let code = run_once();
            self.base_mut().quit_ex().exit_code(code).done();
        }
    }
}

/// Runs the request named by the trailing command-line arguments.
///
/// Returns `0` on success, `1` for a reported failure, and `2` when no response
/// could be produced at all.
fn run_once() -> i32 {
    let arguments = Os::singleton().get_cmdline_user_args();
    if arguments.len() != 2 {
        eprintln!("Themosis runner expects REQUEST_JSON RESPONSE_JSON after --");
        return 2;
    }
    let (Some(request_path), Some(response_path)) = (arguments.get(0), arguments.get(1)) else {
        eprintln!("Themosis runner expects REQUEST_JSON RESPONSE_JSON after --");
        return 2;
    };
    let request_path = PathBuf::from(request_path.to_string());
    let response_path = PathBuf::from(response_path.to_string());
    let version = godot_version();
    let response = match fs::read_to_string(&request_path) {
        Ok(text) => match RunnerRequest::from_json(&text) {
            Ok(request) => handle(request, version),
            Err(error) => RunnerResponse::failure(
                version,
                vec![RunnerDiagnostic::new("invalid_request", error)],
            ),
        },
        Err(error) => RunnerResponse::failure(
            version,
            vec![RunnerDiagnostic::new(
                "request_read",
                format!(
                    "could not read runner request '{}': {error}",
                    request_path.display()
                ),
            )],
        ),
    };
    if let Err(error) = fs::write(&response_path, response.to_json()) {
        eprintln!(
            "Themosis runner could not write response '{}': {error}",
            response_path.display()
        );
        return 2;
    }
    if response.ok { 0 } else { 1 }
}

/// Handles a parsed request and returns the response to write.
fn handle(request: RunnerRequest, version: GodotVersion) -> RunnerResponse {
    if request.schema_version != RUNNER_SCHEMA_VERSION {
        return RunnerResponse::failure(
            version,
            vec![RunnerDiagnostic::new(
                "protocol_version_mismatch",
                format!(
                    "runner protocol version {} is not supported; expected {}",
                    request.schema_version, RUNNER_SCHEMA_VERSION
                ),
            )],
        );
    }
    if (version.major, version.minor, version.patch) < (4, 5, 0) {
        let message = format!(
            "Themosis requires Godot 4.5.0 or newer, got {}",
            version.display
        );
        return RunnerResponse::failure(
            version,
            vec![RunnerDiagnostic::new("unsupported_godot_version", message)],
        );
    }
    if let Some(required) = &request.required_godot_version
        && required != &version.numeric()
    {
        let message = format!("required Godot {required}, got {}", version.display);
        return RunnerResponse::failure(
            version,
            vec![RunnerDiagnostic::new("godot_version_mismatch", message)],
        );
    }
    if request
        .source
        .strip_prefix("res://")
        .filter(|relative| valid_relative_path(relative))
        .is_none()
    {
        return RunnerResponse::failure(
            version,
            vec![RunnerDiagnostic::new(
                "invalid_source",
                format!(
                    "theme source '{}' must be a confined res:// path",
                    request.source
                ),
            )],
        );
    }

    // Compilation, native mapping, and diagnostics come from the shared
    // generation service, so the runner and the editor report one behavior.
    let attempt = generate_from_project_path(&request.source);
    let theme = match attempt.result {
        Ok(theme) => theme,
        Err(failure) => return RunnerResponse::failure(version, failure.diagnostics),
    };
    match request.operation {
        RunnerOperation::Check => RunnerResponse::success(version, None),
        RunnerOperation::Build => {
            let Some(output) = request.output else {
                return RunnerResponse::failure(
                    version,
                    vec![RunnerDiagnostic::new(
                        "invalid_output",
                        "build operation requires an output res:// path",
                    )],
                );
            };
            if let Err(diagnostic) = save_theme(&theme, &output) {
                return RunnerResponse::failure(version, vec![*diagnostic]);
            }
            RunnerResponse::success(version, Some(output))
        }
    }
}

/// Reads the running engine's version components.
fn godot_version() -> GodotVersion {
    let info = Engine::singleton().get_version_info();
    GodotVersion {
        display: dictionary_string(&info, "string").unwrap_or_else(|| "unknown".to_owned()),
        major: dictionary_u32(&info, "major"),
        minor: dictionary_u32(&info, "minor"),
        patch: dictionary_u32(&info, "patch"),
        status: dictionary_string(&info, "status").unwrap_or_default(),
        build: dictionary_string(&info, "build").unwrap_or_default(),
        hash: dictionary_string(&info, "hash").unwrap_or_default(),
    }
}

fn dictionary_string(dictionary: &VarDictionary, key: &str) -> Option<String> {
    dictionary
        .get(key)
        .and_then(|value| value.try_to::<GString>().ok())
        .map(|value| value.to_string())
}

fn dictionary_u32(dictionary: &VarDictionary, key: &str) -> u32 {
    dictionary
        .get(key)
        .and_then(|value| value.try_to::<i64>().ok())
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(0)
}
