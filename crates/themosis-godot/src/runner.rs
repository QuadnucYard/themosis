//! Shared request/response protocol for the native Godot CLI runner.
//!
//! `themosis-cli` writes a [`RunnerRequest`] to a temporary file, starts a Godot
//! project that ships the Themosis addon with `--main-loop ThemosisCliRunner`,
//! and reads the [`RunnerResponse`] the runner writes back. Both sides build
//! these types from this crate, so the wire format has one definition and an
//! explicit version that is independent of the crate versions.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Wire-format version of the [`RunnerRequest`]/[`RunnerResponse`] pair.
///
/// This is deliberately separate from [`crate::GodotBuildPlan`]'s schema
/// version: a runner and a CLI can share a build-plan schema while disagreeing
/// about this protocol, and the mismatch must be reported rather than guessed.
pub const RUNNER_SCHEMA_VERSION: u64 = 1;

/// Operation the runner performs for one source root.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunnerOperation {
    /// Compile and validate the source without writing an artifact.
    Check,
    /// Compile, validate, and save a native theme resource.
    Build,
}

/// One CLI request for the native Godot runner.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RunnerRequest {
    /// Wire-format version; the runner accepts only [`RUNNER_SCHEMA_VERSION`].
    pub schema_version: u64,
    /// Operation to perform.
    pub operation: RunnerOperation,
    /// Root source inside the project, spelled as a `res://` path.
    pub source: String,
    /// Output resource path, required by [`RunnerOperation::Build`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// Exact `MAJOR.MINOR.PATCH` the running engine must match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_godot_version: Option<String>,
}

impl RunnerRequest {
    /// Serializes the request as pretty JSON ending in a newline.
    ///
    /// # Panics
    ///
    /// Panics only if a field stops being serializable, which would be a bug in
    /// this crate rather than caller input.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut json = serde_json::to_string_pretty(self)
            .expect("runner requests contain only string and version fields");
        json.push('\n');
        json
    }

    /// Parses a request, reporting the JSON failure as text.
    pub fn from_json(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|error| format!("invalid runner request: {error}"))
    }
}

/// Runtime version information reported by the running Godot engine.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GodotVersion {
    /// Full engine version string, such as `4.5.1.stable.official`.
    pub display: String,
    /// Major version component.
    pub major: u32,
    /// Minor version component.
    pub minor: u32,
    /// Patch version component.
    pub patch: u32,
    /// Release status, such as `stable`, `beta`, or `rc`.
    pub status: String,
    /// Build identifier supplied by the engine.
    pub build: String,
    /// Commit hash supplied by the engine.
    pub hash: String,
}

impl GodotVersion {
    /// Returns the numeric `MAJOR.MINOR.PATCH` components as text.
    #[must_use]
    pub fn numeric(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }

    /// Returns the display string with an abbreviated commit hash when present.
    #[must_use]
    pub fn label(&self) -> String {
        if self.hash.is_empty() {
            self.display.clone()
        } else {
            let hash = self.hash.chars().take(9).collect::<String>();
            format!("{} [{hash}]", self.display)
        }
    }
}

/// A one-based line and column within a source file.
///
/// Displays as `LINE:COLUMN`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LineColumn {
    /// One-based source line.
    pub line: usize,
    /// One-based source column.
    pub column: usize,
}

impl fmt::Display for LineColumn {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.line, self.column)
    }
}

/// A half-open UTF-8 byte range within a source file.
///
/// Displays as `START..END`. The fields keep the flat `span_start`/`span_end`
/// wire names of the runner protocol; only the in-memory shape groups the
/// range.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceSpan {
    /// Inclusive first byte of the range.
    pub start: usize,
    /// Exclusive end byte of the range.
    pub end: usize,
}

impl fmt::Display for SourceSpan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}..{}", self.start, self.end)
    }
}

/// One structured runner or mapping diagnostic.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RunnerDiagnostic {
    /// Stable symbolic diagnostic code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
    /// Root-relative source path, when the diagnostic has a source location.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,
    /// One-based source line and column, when the diagnostic has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<LineColumn>,
    /// Half-open UTF-8 byte range of the failing source, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<SourceSpan>,
    /// Compiled style name, for native mapping failures.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub style: String,
    /// Native control target, for native mapping failures.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub target: String,
    /// Source state, when the failing item came from a state block.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub state: String,
    /// Native theme-item name, for native mapping failures.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub property: String,
}

impl RunnerDiagnostic {
    /// Creates a diagnostic with only a code and message.
    #[must_use]
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            path: String::new(),
            location: None,
            span: None,
            style: String::new(),
            target: String::new(),
            state: String::new(),
            property: String::new(),
        }
    }

    /// Attaches a root-relative source path.
    #[must_use]
    pub fn at_source(mut self, path: impl Into<String>) -> Self {
        self.path = path.into();
        self
    }

    /// Attaches a one-based line and column within the source path.
    #[must_use]
    pub fn at_line_column(mut self, location: LineColumn) -> Self {
        self.location = Some(location);
        self
    }

    /// Attaches the half-open UTF-8 byte range of the failing source.
    #[must_use]
    pub fn with_span(mut self, span: SourceSpan) -> Self {
        self.span = Some(span);
        self
    }

    /// Attaches the native theme-item context of a mapping failure.
    #[must_use]
    pub fn at_item(
        mut self,
        style: impl Into<String>,
        target: impl Into<String>,
        state: impl Into<String>,
        property: impl Into<String>,
    ) -> Self {
        self.style = style.into();
        self.target = target.into();
        self.state = state.into();
        self.property = property.into();
        self
    }

    /// Renders the diagnostic for terminal output.
    #[must_use]
    pub fn render(&self) -> String {
        let mut context = Vec::new();
        if !self.path.is_empty() {
            let mut location = self.path.clone();
            if let Some(line_column) = self.location {
                location.push_str(&format!(":{line_column}"));
            } else if let Some(span) = self.span {
                location.push_str(&format!(" at bytes {span}"));
            }
            context.push(format!("path={location}"));
        }
        for (field, value) in [
            ("style", &self.style),
            ("target", &self.target),
            ("state", &self.state),
            ("property", &self.property),
        ] {
            if !value.is_empty() {
                context.push(format!("{field}={value}"));
            }
        }
        if context.is_empty() {
            format!("[{}] {}", self.code, self.message)
        } else {
            format!("[{} {}] {}", self.code, context.join(" "), self.message)
        }
    }
}

/// One runner result returned to the CLI.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RunnerResponse {
    /// Wire-format version; always [`RUNNER_SCHEMA_VERSION`].
    pub schema_version: u64,
    /// Whether the operation completed successfully.
    pub ok: bool,
    /// Version of the engine that produced the response.
    pub godot_version: GodotVersion,
    /// Materialized `res://` output path for a successful build.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// Every failure discovered while handling the request.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<RunnerDiagnostic>,
}

impl RunnerResponse {
    /// Creates a successful response.
    #[must_use]
    pub fn success(godot_version: GodotVersion, output: Option<String>) -> Self {
        Self {
            schema_version: RUNNER_SCHEMA_VERSION,
            ok: true,
            godot_version,
            output,
            diagnostics: Vec::new(),
        }
    }

    /// Creates a failed response carrying at least one diagnostic.
    #[must_use]
    pub fn failure(godot_version: GodotVersion, diagnostics: Vec<RunnerDiagnostic>) -> Self {
        Self {
            schema_version: RUNNER_SCHEMA_VERSION,
            ok: false,
            godot_version,
            output: None,
            diagnostics,
        }
    }

    /// Serializes the response as pretty JSON ending in a newline.
    ///
    /// # Panics
    ///
    /// Panics only if a field stops being serializable, which would be a bug in
    /// this crate rather than caller input.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut json = serde_json::to_string_pretty(self)
            .expect("runner responses contain only serializable fields");
        json.push('\n');
        json
    }

    /// Parses a response, reporting the JSON failure as text.
    pub fn from_json(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|error| format!("invalid runner response: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        GodotVersion, RUNNER_SCHEMA_VERSION, RunnerDiagnostic, RunnerOperation, RunnerRequest,
        RunnerResponse,
    };

    fn version() -> GodotVersion {
        GodotVersion {
            display: "4.5.1.stable.official".to_owned(),
            major: 4,
            minor: 5,
            patch: 1,
            status: "stable".to_owned(),
            build: "official".to_owned(),
            hash: "0123456789abcdef".to_owned(),
        }
    }

    #[test]
    fn requests_round_trip_without_absent_fields() {
        let request = RunnerRequest {
            schema_version: RUNNER_SCHEMA_VERSION,
            operation: RunnerOperation::Build,
            source: "res://theme.kdl".to_owned(),
            output: Some("res://generated/theme.tres".to_owned()),
            required_godot_version: None,
        };
        let json = request.to_json();
        assert!(!json.contains("required_godot_version"));
        assert_eq!(
            RunnerRequest::from_json(&json).expect("round trip"),
            request
        );
    }

    #[test]
    fn responses_round_trip_with_diagnostics() {
        let response = RunnerResponse {
            schema_version: RUNNER_SCHEMA_VERSION,
            ok: false,
            godot_version: version(),
            output: None,
            diagnostics: vec![
                RunnerDiagnostic::new("unknown_target", "no such control")
                    .at_item("Primary", "Button", "", "normal"),
            ],
        };
        let json = response.to_json();
        assert_eq!(
            RunnerResponse::from_json(&json).expect("round trip"),
            response
        );
    }

    #[test]
    fn version_label_abbreviates_the_hash() {
        assert_eq!(version().label(), "4.5.1.stable.official [012345678]");
        assert_eq!(version().numeric(), "4.5.1");
    }
}
