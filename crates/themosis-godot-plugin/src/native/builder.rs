//! Native profile-driven theme building shared by the editor plugin and the
//! headless build entry point.

use godot::{classes::Time, obj::Singleton};
use themosis_godot::{
    profiles::{self, Profile, ProfileConfig},
    reports::{
        BatchOutcome, MaterializePlanError, Operation, OperationOutcome, OutcomeStatus, file_stem,
        plan_materialize_all,
    },
    runner::RunnerDiagnostic,
};

use crate::native::{generation::generate_from_project_path, materialize::save_theme};

/// Builds every enabled profile in a configuration.
pub(crate) fn build_all(config: &ProfileConfig) -> BatchOutcome {
    BatchOutcome::from_results(
        config
            .profiles
            .iter()
            .filter(|profile| profile.enabled)
            .map(|profile| build_profile(profile, true))
            .collect(),
    )
}

/// Builds one named profile from an already validated configuration.
pub(crate) fn build_named(config: &ProfileConfig, profile_name: &str) -> OperationOutcome {
    let Some(profile) = config.find(profile_name) else {
        return invalid(
            Operation::Materialize,
            profile_name,
            "",
            "",
            format!("profile '{profile_name}' does not exist"),
            Vec::new(),
        );
    };
    if !profile.enabled {
        return invalid(
            Operation::Materialize,
            profile_name,
            &profile.source,
            &profile.output,
            format!("profile '{profile_name}' is disabled"),
            vec![profile.source.clone()],
        );
    }
    build_profile(profile, true)
}

/// Compiles one profile, saving its output when requested.
pub(crate) fn build_profile(profile: &Profile, save_output: bool) -> OperationOutcome {
    let operation = if save_output {
        Operation::Materialize
    } else {
        Operation::Validate
    };
    let source = profile.source.as_str();
    let output = profile.output.as_str();
    if let Err(error) = profiles::validate_source_path(source) {
        return invalid(
            operation,
            &profile.name,
            source,
            output,
            format!("source {error}"),
            vec![source.to_owned()],
        );
    }
    if let Err(error) = profiles::validate_output_path(output) {
        return invalid(
            operation,
            &profile.name,
            source,
            output,
            format!("output {error}"),
            vec![source.to_owned()],
        );
    }
    let started = Time::singleton().get_ticks_msec();
    let attempt = generate_from_project_path(source);
    let elapsed = elapsed_since(started);
    let dependencies = attempt.dependency_list();
    let theme = match attempt.result {
        Ok(theme) => theme,
        Err(failure) => {
            return failed(
                operation,
                &profile.name,
                source,
                output,
                failure.message,
                failure.diagnostics,
                dependencies,
                elapsed,
            );
        }
    };
    if !save_output {
        return succeeded(
            operation,
            &profile.name,
            source,
            output,
            OutcomeStatus::Validated,
            dependencies,
            elapsed,
        );
    }
    match save_theme(&theme, output) {
        Ok(()) => succeeded(
            operation,
            &profile.name,
            source,
            output,
            OutcomeStatus::Success,
            dependencies,
            elapsed_since(started),
        ),
        Err(diagnostic) => failed(
            operation,
            &profile.name,
            source,
            output,
            diagnostic.render(),
            vec![*diagnostic],
            dependencies,
            elapsed,
        ),
    }
}

/// Materializes one source to an explicit destination.
pub(crate) fn materialize_source(source: &str, output: &str) -> OperationOutcome {
    build_profile(
        &profiles::new_profile(file_stem(source), source, output),
        true,
    )
}

/// Materializes every source into one directory after preflighting it.
///
/// Destinations keep the source file name, so two roots with the same basename
/// would silently overwrite each other. The directory and every destination
/// collision are checked before anything is written.
pub(crate) fn materialize_all(sources: &[String], directory: &str) -> BatchOutcome {
    match plan_materialize_all(sources, directory) {
        Ok(planned) => BatchOutcome::from_results(
            planned
                .iter()
                .map(|destination| materialize_source(&destination.source, &destination.output))
                .collect(),
        ),
        Err(MaterializePlanError::Directory(error)) => {
            BatchOutcome::rejected(format!("output directory {error}"))
        }
        Err(MaterializePlanError::Collisions(collisions)) => {
            BatchOutcome::collided(sources.len(), collisions)
        }
    }
}

/// Builds an outcome for an operation that never reached compilation.
fn invalid(
    operation: Operation,
    profile: &str,
    source: &str,
    output: &str,
    message: String,
    dependencies: Vec<String>,
) -> OperationOutcome {
    failed(
        operation,
        profile,
        source,
        output,
        message.clone(),
        vec![RunnerDiagnostic::new("", message)],
        dependencies,
        0,
    )
}

/// Builds a failure outcome without timing.
#[allow(clippy::too_many_arguments)]
fn failed(
    operation: Operation,
    profile: &str,
    source: &str,
    output: &str,
    error: String,
    diagnostics: Vec<RunnerDiagnostic>,
    dependencies: Vec<String>,
    elapsed_ms: i64,
) -> OperationOutcome {
    OperationOutcome {
        operation,
        profile: profile.to_owned(),
        source: source.to_owned(),
        output: output.to_owned(),
        status: OutcomeStatus::Failure,
        error,
        diagnostics,
        dependencies,
        elapsed_ms,
    }
}

/// Builds a success outcome.
#[allow(clippy::too_many_arguments)]
fn succeeded(
    operation: Operation,
    profile: &str,
    source: &str,
    output: &str,
    status: OutcomeStatus,
    dependencies: Vec<String>,
    elapsed_ms: i64,
) -> OperationOutcome {
    OperationOutcome {
        operation,
        profile: profile.to_owned(),
        source: source.to_owned(),
        output: output.to_owned(),
        status,
        error: String::new(),
        diagnostics: Vec::new(),
        dependencies,
        elapsed_ms,
    }
}

fn elapsed_since(started: u64) -> i64 {
    Time::singleton().get_ticks_msec().saturating_sub(started) as i64
}
