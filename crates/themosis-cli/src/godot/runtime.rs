//! Runtime-backed Godot execution.

use std::{
    env, fs,
    fs::File,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

use clap::Args;
use serde_json::{Value, json};
use tempfile::{Builder as TempDirBuilder, TempDir};
use themosis_godot::{GodotBuildPlan, NATIVE_THEME_BUILDER_GDSCRIPT, NATIVE_THEME_RUNNER_GDSCRIPT};

use super::output::localize_output;

/// Godot runtime selection shared by targeted commands.
#[derive(Debug, Args)]
pub(crate) struct RuntimeOptions {
    /// Godot executable used for native target validation and generation.
    #[arg(long, value_name = "FILE")]
    godot: Option<PathBuf>,
    /// Godot project directory; inferred from the root source when omitted.
    #[arg(long, value_name = "DIR")]
    project: Option<PathBuf>,
    /// Fail unless Godot's numeric version exactly matches MAJOR.MINOR.PATCH.
    #[arg(long, value_name = "MAJOR.MINOR.PATCH", value_parser = parse_required_version)]
    require_godot_version: Option<String>,
    /// Maximum time allowed for the headless Godot operation.
    #[arg(
        long,
        value_name = "SECONDS",
        default_value_t = 120,
        value_parser = clap::value_parser!(u64).range(1..)
    )]
    godot_timeout: u64,
}

impl RuntimeOptions {
    pub(crate) fn check(&self, root: &Path, plan: &GodotBuildPlan) -> Result<String, String> {
        let project = self.project_root(root)?;
        self.run(&project, plan, "check", None)
    }

    pub(crate) fn build(
        &self,
        root: &Path,
        output: &Path,
        plan: &GodotBuildPlan,
    ) -> Result<String, String> {
        let project = self.project_root(root)?;
        let output = localize_output(&project, output)?;
        self.run(&project, plan, "build", Some(&output))
    }

    fn project_root(&self, root: &Path) -> Result<PathBuf, String> {
        if let Some(project) = &self.project {
            return validate_project(project);
        }
        let canonical = root
            .canonicalize()
            .map_err(|error| format!("cannot open '{}': {error}", root.display()))?;
        for ancestor in canonical.ancestors().skip(1) {
            if ancestor.join("project.godot").is_file() {
                return Ok(ancestor.to_path_buf());
            }
        }
        Err(format!(
            "cannot find project.godot above '{}'; pass --project DIR",
            root.display()
        ))
    }

    fn run(
        &self,
        project: &Path,
        plan: &GodotBuildPlan,
        operation: &str,
        output: Option<&str>,
    ) -> Result<String, String> {
        let files = RunnerFiles::create(
            plan,
            operation,
            output,
            self.require_godot_version.as_deref(),
        )?;
        let mut last_missing = None;
        for executable in self.executables() {
            match run_godot(
                &executable,
                project,
                &files,
                Duration::from_secs(self.godot_timeout),
            ) {
                Ok(process) => return parse_response(process, &files),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    last_missing = Some(executable);
                }
                Err(error) => {
                    return Err(format!(
                        "cannot start Godot executable '{}': {error}",
                        executable.display()
                    ));
                }
            }
        }
        let attempted = last_missing.map_or_else(
            || "configured executable".to_owned(),
            |path| format!("'{}'", path.display()),
        );
        Err(format!(
            "cannot find Godot executable {attempted}; pass --godot FILE or set THEMOSIS_GODOT_BINARY"
        ))
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
    builder: PathBuf,
    script: PathBuf,
    request: PathBuf,
    response: PathBuf,
    log: PathBuf,
    stdout: PathBuf,
    stderr: PathBuf,
}

impl RunnerFiles {
    fn create(
        plan: &GodotBuildPlan,
        operation: &str,
        output: Option<&str>,
        required_version: Option<&str>,
    ) -> Result<Self, String> {
        let directory = TempDirBuilder::new()
            .prefix("themosis-godot-")
            .tempdir()
            .map_err(|error| format!("cannot create temporary Godot runner: {error}"))?;
        let files = Self {
            builder: directory.path().join("native_theme_builder.gd"),
            script: directory.path().join("native_theme_runner.gd"),
            request: directory.path().join("request.json"),
            response: directory.path().join("response.json"),
            log: directory.path().join("godot.log"),
            stdout: directory.path().join("stdout.log"),
            stderr: directory.path().join("stderr.log"),
            _directory: directory,
        };
        fs::write(&files.builder, NATIVE_THEME_BUILDER_GDSCRIPT).map_err(|error| {
            format!(
                "cannot write native Godot builder '{}': {error}",
                files.builder.display()
            )
        })?;
        fs::write(&files.script, NATIVE_THEME_RUNNER_GDSCRIPT).map_err(|error| {
            format!(
                "cannot write native Godot runner '{}': {error}",
                files.script.display()
            )
        })?;
        let request = json!({
            "operation": operation,
            "output": output,
            "required_godot_version": required_version,
            "plan": plan,
        });
        let mut request = serde_json::to_string_pretty(&request)
            .expect("Godot runner requests contain serializable values");
        request.push('\n');
        fs::write(&files.request, request).map_err(|error| {
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
        .arg("--script")
        .arg(&files.script)
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

fn parse_response(process: ProcessOutput, files: &RunnerFiles) -> Result<String, String> {
    if process.timed_out {
        return Err(format!(
            "Godot target operation timed out\n{}",
            process_details(&process, files)
        ));
    }
    let response = fs::read_to_string(&files.response).map_err(|error| {
        format!(
            "Godot did not return a build response: {error}\n{}",
            process_details(&process, files)
        )
    })?;
    let response: Value = serde_json::from_str(&response).map_err(|error| {
        format!(
            "Godot returned an invalid build response: {error}\n{}",
            process_details(&process, files)
        )
    })?;
    if response.get("ok").and_then(Value::as_bool) == Some(true) && process.status.success() {
        return version_label(&response)
            .ok_or_else(|| "Godot response did not identify its runtime version".to_owned());
    }
    let diagnostics = response
        .get("diagnostics")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(format_diagnostic)
        .collect::<Vec<_>>();
    if diagnostics.is_empty() {
        Err(format!(
            "Godot target operation failed with {}\n{}",
            process.status,
            process_details(&process, files)
        ))
    } else {
        Err(diagnostics.join("\n"))
    }
}

fn version_label(response: &Value) -> Option<String> {
    let version = response.get("godot_version")?;
    if let Some(version) = version.as_str() {
        return Some(version.to_owned());
    }
    let display = version.get("display")?.as_str()?;
    let hash = version
        .get("hash")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if hash.is_empty() {
        Some(display.to_owned())
    } else {
        Some(format!(
            "{display} [{}]",
            hash.chars().take(9).collect::<String>()
        ))
    }
}

fn format_diagnostic(diagnostic: &Value) -> String {
    let message = diagnostic
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("Godot target operation failed");
    let code = diagnostic
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or("godot_error");
    let context = ["style", "target", "state", "property"]
        .into_iter()
        .filter_map(|field| {
            diagnostic
                .get(field)
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(|value| format!("{field}={value}"))
        })
        .collect::<Vec<_>>();
    if context.is_empty() {
        format!("[{code}] {message}")
    } else {
        format!("[{code} {}] {message}", context.join(" "))
    }
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
