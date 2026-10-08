//! Portable state machine and presentation behind the native Themosis dock.
//!
//! The dock itself is only a renderer: it maps [`EditorView`] onto Godot
//! controls and forwards UI events. Statuses, selection, operation tracking,
//! preview choice, and diagnostics text are decided here, so they are testable
//! without a running engine.

use std::collections::BTreeMap;

use crate::{
    reports::{BatchOutcome, Operation, OperationOutcome, file_name},
    runner::RunnerDiagnostic,
};

/// Freshness of one discovered theme root.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Status {
    /// Discovered but no import has been observed yet.
    #[default]
    Unverified,
    /// An import is currently running.
    Importing,
    /// A dependency changed on disk since the last import.
    Stale,
    /// The last import failed.
    Failed,
    /// The last import matches the current sources.
    UpToDate,
}

impl Status {
    /// Returns the label shown next to the source in the dock selector.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Unverified => "unverified",
            Self::Importing => "importing",
            Self::Stale => "stale",
            Self::Failed => "failed",
            Self::UpToDate => "up-to-date",
        }
    }

    /// Returns whether a retained theme must be labelled as the last valid one.
    fn retains_preview(self) -> bool {
        matches!(
            self,
            Self::Unverified | Self::Importing | Self::Stale | Self::Failed
        )
    }
}

/// Visual tone the dock applies to its status line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tone {
    /// No sources are discovered.
    Neutral,
    /// The current state is healthy.
    Success,
    /// The state needs attention but is not broken.
    Warning,
    /// The last operation failed.
    Failure,
    /// An import is running.
    Importing,
}

/// Theme the dock shows in its preview panel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Preview {
    /// No source is selected.
    Empty,
    /// The selected source has no valid theme yet.
    Missing,
    /// The theme of the source's current import.
    Current,
    /// The retained theme of an earlier successful operation.
    LastValid,
    /// The theme of the last successful materialization.
    Materialized,
    /// The theme of the last successful validation.
    Generated,
}

impl Preview {
    /// Returns whether the dock must assign a theme to its preview panel.
    #[must_use]
    pub fn shows_theme(self) -> bool {
        !matches!(self, Self::Empty | Self::Missing)
    }

    /// Returns the preview title, falling back to the source file name when the
    /// theme resource has no name.
    #[must_use]
    pub fn title(self, theme_name: &str, source_file: &str) -> String {
        let name = if theme_name.is_empty() {
            source_file
        } else {
            theme_name
        };
        match self {
            Self::Empty => "Theme preview".to_owned(),
            Self::Missing => "No valid preview".to_owned(),
            Self::Current => name.to_owned(),
            Self::LastValid => format!("Last valid preview: {name}"),
            Self::Materialized => format!("Materialized preview: {name}"),
            Self::Generated => format!("Generated preview: {name}"),
        }
    }
}

/// One rendered diagnostics line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticLine {
    /// `res://` path the line selects when clicked; empty when it has none.
    pub meta_path: String,
    /// Rendered line text.
    pub text: String,
}

/// Everything the dock renders for the currently selected source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorView {
    /// Status line text.
    pub status_text: String,
    /// Tone for the status line.
    pub tone: Tone,
    /// Diagnostics of the last operation.
    pub diagnostics: Vec<DiagnosticLine>,
    /// Preview panel content.
    pub preview: Preview,
}

/// Status and diagnostics of one bulk materialization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchView {
    /// Status line text.
    pub status_text: String,
    /// Tone for the status line.
    pub tone: Tone,
    /// Diagnostics produced by the batch.
    pub diagnostics: Vec<DiagnosticLine>,
}

/// Renders one diagnostic as a dock line.
#[must_use]
pub fn diagnostic_line(diagnostic: &RunnerDiagnostic) -> DiagnosticLine {
    let mut text = String::new();
    if !diagnostic.path.is_empty() {
        text.push_str(&diagnostic.path);
        if let Some(location) = diagnostic.location {
            text.push_str(&format!(":{location}"));
        } else if let Some(span) = diagnostic.span
            && span.end >= span.start
        {
            text.push_str(&format!(" at bytes {span}"));
        }
        text.push_str(": ");
    }
    let context = [
        ("style", &diagnostic.style),
        ("target", &diagnostic.target),
        ("state", &diagnostic.state),
        ("property", &diagnostic.property),
    ]
    .iter()
    .filter(|(_, value)| !value.is_empty())
    .map(|(key, value)| format!("{key}={value}"))
    .collect::<Vec<_>>();
    if !context.is_empty() {
        text.push_str(&format!("[{}] ", context.join(" ")));
    }
    if !diagnostic.code.is_empty() {
        text.push_str(&format!("[{}] ", diagnostic.code));
    }
    text.push_str(if diagnostic.message.is_empty() {
        "error"
    } else {
        diagnostic.message.as_str()
    });
    DiagnosticLine {
        meta_path: diagnostic.path.clone(),
        text,
    }
}

/// Renders one bulk materialization for the dock.
#[must_use]
pub fn batch_view(batch: &BatchOutcome) -> BatchView {
    if !batch.collisions.is_empty() {
        let mut diagnostics = vec![DiagnosticLine {
            meta_path: String::new(),
            text: "Destination collisions:".to_owned(),
        }];
        diagnostics.extend(batch.collisions.iter().map(|collision| DiagnosticLine {
            meta_path: String::new(),
            text: format!("{} <- {}", collision.output, collision.sources.join(", ")),
        }));
        return BatchView {
            status_text: format!(
                "Materialization blocked by {} destination collision(s)",
                batch.collisions.len()
            ),
            tone: Tone::Failure,
            diagnostics,
        };
    }
    if batch.results.is_empty() && !batch.ok() {
        // The directory was rejected before any source ran, so there is no
        // per-source result to report and the batch must not look successful.
        return BatchView {
            status_text: if batch.error.is_empty() {
                "Materialization failed".to_owned()
            } else {
                batch.error.clone()
            },
            tone: Tone::Failure,
            diagnostics: Vec::new(),
        };
    }
    BatchView {
        status_text: format!(
            "Materialized {} theme(s){}",
            batch.outputs().len(),
            if batch.failures() > 0 {
                format!(" with {} failure(s)", batch.failures())
            } else {
                String::new()
            }
        ),
        tone: if batch.ok() {
            Tone::Success
        } else {
            Tone::Failure
        },
        diagnostics: batch
            .results
            .iter()
            .filter(|result| !result.ok())
            .map(|result| DiagnosticLine {
                meta_path: result.source.clone(),
                text: if result.error.is_empty() {
                    "materialization failed".to_owned()
                } else {
                    result.error.clone()
                },
            })
            .collect(),
    }
}

/// Last operation recorded for one source.
#[derive(Clone, Debug, Default)]
struct SourceRecord {
    status: Status,
    outcome: Option<OperationOutcome>,
}

/// Statuses, selection, and recorded outcomes of every discovered root.
#[derive(Clone, Debug, Default)]
pub struct EditorState {
    sources: Vec<String>,
    records: BTreeMap<String, SourceRecord>,
    selected: String,
}

impl EditorState {
    /// Creates an empty state with no discovered sources.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the discovered sources, keeping records for surviving roots.
    pub fn set_sources(&mut self, mut sources: Vec<String>) {
        sources.sort();
        sources.dedup();
        self.sources = sources;
        let discovered = &self.sources;
        self.records.retain(|source, _| discovered.contains(source));
        if !self.selected.is_empty() && !self.sources.contains(&self.selected) {
            self.selected.clear();
        }
        if self.selected.is_empty() && !self.sources.is_empty() {
            self.selected = self.sources[0].clone();
        }
    }

    /// Returns the discovered sources in display order.
    #[must_use]
    pub fn sources(&self) -> &[String] {
        &self.sources
    }

    /// Returns the selected source, or the empty string.
    #[must_use]
    pub fn selected(&self) -> &str {
        &self.selected
    }

    /// Returns a source's status, defaulting to [`Status::Unverified`].
    #[must_use]
    pub fn status(&self, source: &str) -> Status {
        self.records
            .get(source)
            .map_or(Status::Unverified, |record| record.status)
    }

    /// Selects the source at `index`; returns whether the selection changed.
    pub fn select_index(&mut self, index: usize) -> bool {
        if index >= self.sources.len() || self.sources[index] == self.selected {
            return false;
        }
        self.selected = self.sources[index].clone();
        true
    }

    /// Marks a source as currently importing.
    pub fn mark_importing(&mut self, source: &str) {
        self.records.entry(source.to_owned()).or_default().status = Status::Importing;
    }

    /// Marks a source as stale on disk.
    pub fn mark_stale(&mut self, source: &str) {
        self.records.entry(source.to_owned()).or_default().status = Status::Stale;
    }

    /// Records one operation's outcome.
    ///
    /// Only imports change a source's freshness: a successful materialization
    /// does not make a stale import current, and a failed materialization does
    /// not invalidate a successful import.
    pub fn record(&mut self, outcome: OperationOutcome) {
        let record = self.records.entry(outcome.source.clone()).or_default();
        if outcome.operation == Operation::Import {
            record.status = if outcome.ok() {
                Status::UpToDate
            } else {
                Status::Failed
            };
        }
        record.outcome = Some(outcome);
    }

    /// Renders the selected source, using the retained theme when the current
    /// operation produced none.
    #[must_use]
    pub fn view(&self, has_retained_theme: bool) -> EditorView {
        if self.selected.is_empty() {
            return EditorView {
                status_text: "No Themosis theme assets found".to_owned(),
                tone: Tone::Neutral,
                diagnostics: Vec::new(),
                preview: Preview::Empty,
            };
        }
        let source = self.selected.as_str();
        let file = file_name(source);
        let status = self.status(source);
        let outcome = self.records.get(source).and_then(|r| r.outcome.as_ref());
        let (status_text, tone) = match outcome {
            Some(outcome) if outcome.operation != Operation::Import => {
                let tone = if !outcome.ok() {
                    Tone::Failure
                } else if status == Status::UpToDate {
                    Tone::Success
                } else {
                    Tone::Warning
                };
                (
                    format!(
                        "{} {}; import {}",
                        outcome.operation.label(),
                        if outcome.ok() { "succeeded" } else { "failed" },
                        status.label()
                    ),
                    tone,
                )
            }
            _ => import_status(file, status, outcome.is_some()),
        };
        let preview = match outcome {
            Some(outcome) if outcome.ok() => match outcome.operation {
                Operation::Import if status.retains_preview() => Preview::LastValid,
                Operation::Import => Preview::Current,
                Operation::Materialize => Preview::Materialized,
                Operation::Validate => Preview::Generated,
            },
            _ if has_retained_theme => Preview::LastValid,
            _ => Preview::Missing,
        };
        EditorView {
            status_text,
            tone,
            diagnostics: outcome
                .map(|outcome| outcome.diagnostics.iter().map(diagnostic_line).collect())
                .unwrap_or_default(),
            preview,
        }
    }
}

/// Text and tone of a source's import freshness.
fn import_status(file: &str, status: Status, has_result: bool) -> (String, Tone) {
    match status {
        Status::Importing => (format!("Importing {file}…"), Tone::Importing),
        Status::Stale => (
            format!("{file} changed on disk; reimport to refresh"),
            Tone::Warning,
        ),
        Status::Failed => ("Import failed".to_owned(), Tone::Failure),
        Status::Unverified => (format!("{file} import is unverified"), Tone::Warning),
        Status::UpToDate => (
            if has_result {
                format!("Imported {file}")
            } else {
                format!("{file} is ready")
            },
            Tone::Success,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{EditorState, Preview, Status, Tone, batch_view, diagnostic_line};
    use crate::{
        reports::{BatchOutcome, DestinationCollision, Operation, OperationOutcome, OutcomeStatus},
        runner::{LineColumn, RunnerDiagnostic, SourceSpan},
    };

    const ALPHA: &str = "res://theme/alpha.tms";
    const BETA: &str = "res://theme/beta.tms";

    fn outcome(operation: Operation, source: &str, ok: bool, error: &str) -> OperationOutcome {
        OperationOutcome {
            operation,
            profile: String::new(),
            source: source.to_owned(),
            output: String::new(),
            status: if ok {
                OutcomeStatus::Success
            } else {
                OutcomeStatus::Failure
            },
            error: error.to_owned(),
            diagnostics: if ok {
                Vec::new()
            } else {
                vec![RunnerDiagnostic::new("import_failed", error)]
            },
            dependencies: vec![source.to_owned()],
            elapsed_ms: 1,
        }
    }

    #[test]
    fn empty_state_explains_that_no_sources_were_found() {
        let state = EditorState::new();
        assert_eq!(state.selected(), "");
        let view = state.view(false);
        assert_eq!(view.status_text, "No Themosis theme assets found");
        assert_eq!(view.tone, Tone::Neutral);
        assert_eq!(view.preview, Preview::Empty);
        assert!(view.diagnostics.is_empty());
    }

    #[test]
    fn selection_follows_sorted_discovery() {
        let mut state = EditorState::new();
        state.set_sources(vec![BETA.to_owned(), ALPHA.to_owned()]);
        assert_eq!(state.sources(), [ALPHA, BETA]);
        assert_eq!(state.selected(), ALPHA);
        assert_eq!(state.status(ALPHA), Status::Unverified);
        assert!(state.select_index(1));
        assert_eq!(state.selected(), BETA);
        assert!(!state.select_index(1));
    }

    #[test]
    fn imports_move_status_between_up_to_date_and_failed() {
        let mut state = EditorState::new();
        state.set_sources(vec![ALPHA.to_owned()]);
        state.record(outcome(Operation::Import, ALPHA, true, ""));
        assert_eq!(state.status(ALPHA), Status::UpToDate);
        assert_eq!(state.view(true).preview, Preview::Current);
        assert_eq!(state.view(true).status_text, "Imported alpha.tms");
        assert_eq!(state.view(true).tone, Tone::Success);

        state.record(outcome(Operation::Import, ALPHA, false, "alpha regressed"));
        assert_eq!(state.status(ALPHA), Status::Failed);
        let view = state.view(true);
        assert_eq!(view.status_text, "Import failed");
        assert_eq!(view.tone, Tone::Failure);
        assert_eq!(view.preview, Preview::LastValid);
        assert_eq!(view.diagnostics.len(), 1);
        assert!(view.diagnostics[0].text.contains("alpha regressed"));
        assert_eq!(state.view(false).preview, Preview::Missing);
    }

    #[test]
    fn other_operations_do_not_change_import_freshness() {
        let mut state = EditorState::new();
        state.set_sources(vec![ALPHA.to_owned()]);
        state.record(outcome(Operation::Import, ALPHA, true, ""));
        state.mark_stale(ALPHA);

        state.record(outcome(Operation::Materialize, ALPHA, true, ""));
        assert_eq!(state.status(ALPHA), Status::Stale);
        let view = state.view(true);
        assert_eq!(view.status_text, "Materialization succeeded; import stale");
        assert_eq!(view.tone, Tone::Warning);
        assert_eq!(view.preview, Preview::Materialized);

        state.record(outcome(Operation::Validate, ALPHA, true, ""));
        assert_eq!(state.status(ALPHA), Status::Stale);
        assert_eq!(
            state.view(true).status_text,
            "Validation succeeded; import stale"
        );
        assert_eq!(state.view(true).preview, Preview::Generated);

        state.record(outcome(Operation::Materialize, ALPHA, false, "cannot save"));
        assert_eq!(state.status(ALPHA), Status::Stale);
        assert_eq!(
            state.view(true).status_text,
            "Materialization failed; import stale"
        );
        assert_eq!(state.view(true).tone, Tone::Failure);
        assert_eq!(state.view(true).preview, Preview::LastValid);

        // A failed materialization must not invalidate a successful import.
        state.record(outcome(Operation::Import, ALPHA, true, ""));
        state.record(outcome(Operation::Materialize, ALPHA, false, "cannot save"));
        assert_eq!(state.status(ALPHA), Status::UpToDate);
        assert_eq!(
            state.view(true).status_text,
            "Materialization failed; import up-to-date"
        );
    }

    #[test]
    fn stale_and_importing_states_prompt_a_reimport() {
        let mut state = EditorState::new();
        state.set_sources(vec![ALPHA.to_owned()]);
        state.mark_stale(ALPHA);
        assert_eq!(
            state.view(true).status_text,
            "alpha.tms changed on disk; reimport to refresh"
        );
        state.mark_importing(ALPHA);
        assert_eq!(state.view(true).status_text, "Importing alpha.tms…");
        assert_eq!(state.view(true).tone, Tone::Importing);
    }

    #[test]
    fn preview_titles_name_the_shown_theme() {
        assert_eq!(Preview::Empty.title("", "alpha.tms"), "Theme preview");
        assert_eq!(
            Preview::Missing.title("alpha", "alpha.tms"),
            "No valid preview"
        );
        assert_eq!(Preview::Current.title("alpha", "alpha.tms"), "alpha");
        assert_eq!(Preview::Current.title("", "alpha.tms"), "alpha.tms");
        assert_eq!(
            Preview::LastValid.title("beta", "beta.tms"),
            "Last valid preview: beta"
        );
        assert!(Preview::Current.shows_theme());
        assert!(!Preview::Missing.shows_theme());
    }

    #[test]
    fn batch_view_reports_rejections_collisions_and_failures() {
        let rejected = batch_view(&BatchOutcome::rejected("output directory rejected"));
        assert_eq!(rejected.status_text, "output directory rejected");
        assert_eq!(rejected.tone, Tone::Failure);
        assert!(rejected.diagnostics.is_empty());

        let collided = batch_view(&BatchOutcome::collided(
            2,
            vec![DestinationCollision {
                output: "res://generated/alpha.tres".to_owned(),
                sources: vec![ALPHA.to_owned(), BETA.to_owned()],
            }],
        ));
        assert!(collided.status_text.contains("collision"));
        assert_eq!(collided.diagnostics[0].text, "Destination collisions:");
        assert!(
            collided.diagnostics[1]
                .text
                .contains("res://generated/alpha.tres")
        );

        let partial = batch_view(&BatchOutcome::from_results(vec![
            outcome(
                Operation::Materialize,
                ALPHA,
                false,
                "alpha failed to compile",
            ),
            outcome(Operation::Materialize, BETA, true, ""),
        ]));
        assert_eq!(
            partial.status_text,
            "Materialized 0 theme(s) with 1 failure(s)"
        );
        assert_eq!(partial.tone, Tone::Failure);
        assert_eq!(partial.diagnostics[0].text, "alpha failed to compile");
    }

    #[test]
    fn batch_view_counts_materialized_themes_and_clears_diagnostics() {
        let empty = batch_view(&BatchOutcome::from_results(Vec::new()));
        assert_eq!(empty.status_text, "Materialized 0 theme(s)");
        assert_eq!(empty.tone, Tone::Success);
        assert!(empty.diagnostics.is_empty());

        let mut first = outcome(Operation::Materialize, ALPHA, true, "");
        first.output = "res://generated/alpha.tres".to_owned();
        let mut second = outcome(Operation::Materialize, BETA, true, "");
        second.output = "res://generated/beta.tres".to_owned();
        let complete = batch_view(&BatchOutcome::from_results(vec![first, second]));
        assert_eq!(complete.status_text, "Materialized 2 theme(s)");
        assert_eq!(complete.tone, Tone::Success);
        assert!(complete.diagnostics.is_empty());
    }

    #[test]
    fn diagnostics_render_paths_locations_and_native_context() {
        let diagnostic = RunnerDiagnostic::new("unsupported_property", "boolean is unsupported")
            .at_source("res://theme/styles/buttons.kdl")
            .at_line_column(LineColumn { line: 3, column: 5 })
            .at_item("PrimaryButton", "Button", "hover", "normal");
        let line = diagnostic_line(&diagnostic);
        assert_eq!(line.meta_path, "res://theme/styles/buttons.kdl");
        assert_eq!(
            line.text,
            "res://theme/styles/buttons.kdl:3:5: [style=PrimaryButton target=Button state=hover property=normal] [unsupported_property] boolean is unsupported"
        );

        let mut spanned = RunnerDiagnostic::new("kdl_syntax", "unexpected token")
            .with_span(SourceSpan { start: 4, end: 9 });
        assert_eq!(
            diagnostic_line(&spanned).text,
            "[kdl_syntax] unexpected token"
        );
        spanned.path = "res://theme/light.tms".to_owned();
        assert_eq!(
            diagnostic_line(&spanned).text,
            "res://theme/light.tms at bytes 4..9: [kdl_syntax] unexpected token"
        );
    }
}
