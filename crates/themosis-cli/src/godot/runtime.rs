//! Project-aware Godot runner execution.

use std::{
    env, fmt, fs,
    fs::File,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

use clap::Args;
use tempfile::{Builder as TempDirBuilder, TempDir};
use themosis_godot::{
    GodotVersion, RUNNER_SCHEMA_VERSION, RunnerDiagnostic, RunnerOperation, RunnerRequest,
    RunnerResponse,
};

use super::{output::localize_output, source::localize_source};

/// Godot runtime selection shared by the Godot subcommands.
#[derive(Debug, Args)]
pub(crate) struct RuntimeOptions {
    /// Godot executable used for native validation and generation.
    #[arg(long, value_name = "FILE")]
    godot: Option<PathBuf>,
    /// Godot project directory; inferred from the source or current directory.
    #[arg(long, value_name = "DIR")]
    project: Option<PathBuf>,
    /// Fail unless Godot's numeric version exactly matches MAJOR.MINOR.PATCH.
    #[arg(long, value_name = "MAJOR.MINOR.PATCH", value_parser = parse_required_version)]
    require_version: Option<String>,
    /// Maximum time allowed for the headless Godot operation.
    #[arg(
        long,
        value_name = "SECONDS",
        default_value_t = 120,
        value_parser = clap::value_parser!(u64).range(1..)
    )]
    timeout: u64,
}

/// Failure of a project-aware Godot operation.
#[derive(Debug)]
pub(crate) enum RunError {
    /// The runner reported structured diagnostics.
    Diagnostics(Vec<RunnerDiagnostic>),
    /// No usable runner response was produced.
    Message(String),
}

impl fmt::Display for RunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Diagnostics(diagnostics) => {
                let rendered = diagnostics
                    .iter()
                    .map(RunnerDiagnostic::render)
                    .collect::<Vec<_>>()
                    .join("\n");
                formatter.write_str(&rendered)
            }
            Self::Message(message) => formatter.write_str(message),
        }
    }
}

impl RuntimeOptions {
    /// Compiles and maps a theme root without writing an artifact.
    pub(crate) fn check(&self, root: &Path) -> Result<GodotVersion, RunError> {
        self.execute(root, RunnerOperation::Check, None)
            .map(|(version, _)| version)
    }

    /// Compiles a theme root and writes its native theme, returning the output path.
    pub(crate) fn build(
        &self,
        root: &Path,
        output: &Path,
    ) -> Result<(GodotVersion, String), RunError> {
        self.execute(root, RunnerOperation::Build, Some(output))
            .and_then(|(version, output)| {
                output
                    .map(|output| (version, output))
                    .ok_or_else(|| RunError::Message("runner reported no output path".to_owned()))
            })
    }

    fn execute(
        &self,
        root: &Path,
        operation: RunnerOperation,
        output: Option<&Path>,
    ) -> Result<(GodotVersion, Option<String>), RunError> {
        let project = self.project_root(root).map_err(RunError::Message)?;
        let source = localize_source(&project, root).map_err(RunError::Message)?;
        let output = match output {
            Some(output) => Some(localize_output(&project, output).map_err(RunError::Message)?),
            None => None,
        };
        require_addon(&project)?;
        let request = RunnerRequest {
            schema_version: RUNNER_SCHEMA_VERSION,
            operation,
            source,
            output,
            required_godot_version: self.require_version.clone(),
        };
        self.run(&project, &request)
            .map(|response| (response.godot_version, response.output))
    }

    fn project_root(&self, root: &Path) -> Result<PathBuf, String> {
        if let Some(project) = &self.project {
            return validate_project(project);
        }
        if !root.to_string_lossy().starts_with("res://")
            && let Ok(canonical) = root.canonicalize()
            && let Some(ancestor) = canonical
                .ancestors()
                .skip(1)
                .find(|ancestor| ancestor.join("project.godot").is_file())
        {
            return validate_project(ancestor);
        }
        let current = env::current_dir()
            .map_err(|error| format!("cannot resolve current directory: {error}"))?;
        if current.join("project.godot").is_file() {
            return validate_project(&current);
        }
        Err(format!(
            "cannot find project.godot above '{}' or in the current directory; pass --project DIR",
            root.display()
        ))
    }

    fn run(&self, project: &Path, request: &RunnerRequest) -> Result<RunnerResponse, RunError> {
        let files = RunnerFiles::create(request).map_err(RunError::Message)?;
        let mut last_missing = None;
        for executable in self.executables() {
            match run_godot(
                &executable,
                project,
                &files,
                Duration::from_secs(self.timeout),
            ) {
                Ok(process) => return parse_response(process, &files),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    last_missing = Some(executable);
                }
                Err(error) => {
                    return Err(RunError::Message(format!(
                        "cannot start Godot executable '{}': {error}",
                        executable.display()
                    )));
                }
            }
        }
        let attempted = last_missing.map_or_else(
            || "configured executable".to_owned(),
            |path| format!("'{}'", path.display()),
        );
        Err(RunError::Message(format!(
            "cannot find Godot executable {attempted}; pass --godot FILE or set THEMOSIS_GODOT_BINARY"
        )))
    }

    fn executables(&self) -> Vec<PathBuf> {
        if let Some(executable) = &self.godot {
            return vec![executable.clone()];
        }
        if let Some(executable) = env::var_os("THEMOSIS_GODOT_BINARY")
            && !executable.is_empty()
        {
            return vec![PathBuf::from(executable)];
        }
        vec![PathBuf::from("godot"), PathBuf::from("godot4")]
    }
}

struct RunnerFiles {
    _directory: TempDir,
    scene: PathBuf,
    request: PathBuf,
    response: PathBuf,
    log: PathBuf,
    stdout: PathBuf,
    stderr: PathBuf,
}

impl RunnerFiles {
    fn create(request: &RunnerRequest) -> Result<Self, String> {
        let directory = TempDirBuilder::new()
            .prefix("themosis-godot-")
            .tempdir()
            .map_err(|error| format!("cannot create temporary Godot runner: {error}"))?;
        let files = Self {
            scene: directory.path().join("runner.tscn"),
            request: directory.path().join("request.json"),
            response: directory.path().join("response.json"),
            log: directory.path().join("godot.log"),
            stdout: directory.path().join("stdout.log"),
            stderr: directory.path().join("stderr.log"),
            _directory: directory,
        };
        // SceneTree requires a scene even for a custom main loop. Always supply
        // an inert one so asset compilation never starts the application's scene.
        fs::write(
            &files.scene,
            "[gd_scene format=3]\n[node name=\"Runner\" type=\"Node\"]\n",
        )
        .map_err(|error| format!("cannot write runner scene: {error}"))?;
        fs::write(&files.request, request.to_json()).map_err(|error| {
            format!(
                "cannot write Godot request '{}': {error}",
                files.request.display()
            )
        })?;
        Ok(files)
    }
}

struct ProcessOutput {
    status: ExitStatus,
    timed_out: bool,
    stdout: String,
    stderr: String,
}

fn run_godot(
    executable: &Path,
    project: &Path,
    files: &RunnerFiles,
    timeout: Duration,
) -> std::io::Result<ProcessOutput> {
    let stdout = File::create(&files.stdout)?;
    let stderr = File::create(&files.stderr)?;
    let mut child = Command::new(executable)
        .args(["--headless", "--path"])
        .arg(project)
        .arg("--log-file")
        .arg(&files.log)
        .args(["--main-loop", "ThemosisCliRunner"])
        .arg(&files.scene)
        .arg("--")
        .arg(&files.request)
        .arg(&files.response)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()?;
    let deadline = Instant::now().checked_add(timeout).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Godot timeout exceeds the platform clock range",
        )
    })?;
    let (status, timed_out) = loop {
        if let Some(status) = child.try_wait()? {
            break (status, false);
        }
        if Instant::now() >= deadline {
            if let Err(error) = child.kill() {
                if let Some(status) = child.try_wait()? {
                    break (status, false);
                }
                return Err(error);
            }
            break (child.wait()?, true);
        }
        thread::sleep(Duration::from_millis(20));
    };
    Ok(ProcessOutput {
        status,
        timed_out,
        stdout: fs::read_to_string(&files.stdout).unwrap_or_default(),
        stderr: fs::read_to_string(&files.stderr).unwrap_or_default(),
    })
}

fn parse_response(process: ProcessOutput, files: &RunnerFiles) -> Result<RunnerResponse, RunError> {
    if process.timed_out {
        return Err(RunError::Message(format!(
            "Godot runner timed out\n{}",
            process_details(&process, files)
        )));
    }
    let text = fs::read_to_string(&files.response).map_err(|error| {
        RunError::Message(format!(
            "Godot did not produce a runner response ({error}); ensure a compatible Themosis addon is installed and built for this Godot version\n{}",
            process_details(&process, files)
        ))
    })?;
    let response = RunnerResponse::from_json(&text).map_err(|error| {
        RunError::Message(format!(
            "Godot returned an invalid runner response: {error}\n{}",
            process_details(&process, files)
        ))
    })?;
    if response.schema_version != RUNNER_SCHEMA_VERSION {
        return Err(RunError::Message(format!(
            "Themosis addon uses runner protocol version {}, but this CLI expects {}; reinstall a matching addon",
            response.schema_version, RUNNER_SCHEMA_VERSION
        )));
    }
    if response.ok && process.status.success() {
        return Ok(response);
    }
    if !response.diagnostics.is_empty() {
        return Err(RunError::Diagnostics(response.diagnostics));
    }
    Err(RunError::Message(format!(
        "Godot runner failed with {}\n{}",
        process.status,
        process_details(&process, files)
    )))
}

fn process_details(process: &ProcessOutput, files: &RunnerFiles) -> String {
    let log = fs::read_to_string(&files.log).unwrap_or_default();
    format!(
        "stdout:\n{}\nstderr:\n{}\nGodot log:\n{}",
        process.stdout.trim(),
        process.stderr.trim(),
        log.trim()
    )
}

/// Requires the Themosis GDExtension and a project Godot has already imported.
///
/// Godot only loads GDExtensions listed in `.godot/extension_list.cfg`, which the
/// editor writes during import. Without that registration `--main-loop` falls
/// back to an empty SceneTree, so both a missing manifest and an unimported
/// project are reported before the engine starts instead of hitting the timeout.
fn require_addon(project: &Path) -> Result<(), RunError> {
    let manifest = [
        "themosis.gdextension",
        "addons/themosis/themosis.gdextension",
    ]
    .into_iter()
    .map(|relative| project.join(relative))
    .find(|candidate| candidate.is_file());
    if manifest.is_none() {
        return Err(RunError::Message(format!(
            "Godot project '{}' has no Themosis addon; install it under res://addons/themosis or add themosis.gdextension",
            project.display()
        )));
    }
    let registered = fs::read_to_string(project.join(".godot/extension_list.cfg"))
        .is_ok_and(|text| text.contains("themosis.gdextension"));
    if !registered {
        return Err(RunError::Message(format!(
            "Godot project '{}' has not registered the Themosis addon; import it first with `godot --headless --editor --import --path {}`",
            project.display(),
            project.display()
        )));
    }
    Ok(())
}

fn validate_project(project: &Path) -> Result<PathBuf, String> {
    let project = project.canonicalize().map_err(|error| {
        format!(
            "cannot open Godot project directory '{}': {error}",
            project.display()
        )
    })?;
    if !project.join("project.godot").is_file() {
        return Err(format!(
            "Godot project directory '{}' has no project.godot",
            project.display()
        ));
    }
    Ok(project)
}

fn parse_required_version(value: &str) -> Result<String, String> {
    let components = value.split('.').collect::<Vec<_>>();
    if components.len() == 3
        && components
            .iter()
            .all(|component| !component.is_empty() && component.parse::<u32>().is_ok())
    {
        Ok(value.to_owned())
    } else {
        Err("expected a numeric MAJOR.MINOR.PATCH version such as 4.5.0".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::parse_required_version;

    #[test]
    fn rejects_invalid_version_requirements() {
        assert_eq!(
            parse_required_version("4.5.0").expect("version is valid"),
            "4.5.0"
        );
        assert!(parse_required_version("4.5").is_err());
        assert!(parse_required_version("4.5-stable").is_err());
    }
}
