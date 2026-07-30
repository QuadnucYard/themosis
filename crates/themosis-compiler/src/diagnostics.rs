use std::{collections::BTreeMap, fmt};

use themosis_core::{Diagnostic, Name, SourceId, Span, TokenPath};
use thiserror::Error;

use crate::CompileError;

/// Importance of a source label within one diagnostic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LabelKind {
    /// The declaration directly responsible for the failure.
    Primary,
    /// A related declaration needed to explain the failure.
    Secondary,
}

/// A source-aware label attached to a semantic diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticLabel {
    source: SourceId,
    span: Option<Span>,
    kind: LabelKind,
    message: String,
}

impl DiagnosticLabel {
    pub(crate) fn primary(
        source: SourceId,
        span: Option<Span>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(source, span, LabelKind::Primary, message)
    }

    pub(crate) fn secondary(
        source: SourceId,
        span: Option<Span>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(source, span, LabelKind::Secondary, message)
    }

    fn new(
        source: SourceId,
        span: Option<Span>,
        kind: LabelKind,
        message: impl Into<String>,
    ) -> Self {
        debug_assert!(span.is_none_or(|span| span.source() == source));
        Self {
            source,
            span,
            kind,
            message: message.into(),
        }
    }

    /// Returns the labeled source.
    #[must_use]
    pub const fn source(&self) -> SourceId {
        self.source
    }

    /// Returns the labeled byte span when available.
    #[must_use]
    pub const fn span(&self) -> Option<Span> {
        self.span
    }

    /// Returns whether this is a primary or related label.
    #[must_use]
    pub const fn kind(&self) -> LabelKind {
        self.kind
    }

    /// Returns the label explanation.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Presentation metadata parallel to one [`CompileError`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticMetadata {
    labels: Vec<DiagnosticLabel>,
    suggestion: Option<String>,
}

impl DiagnosticMetadata {
    /// Returns labels in deterministic explanatory order.
    #[must_use]
    pub fn labels(&self) -> &[DiagnosticLabel] {
        &self.labels
    }

    /// Returns a concise reliable suggestion when one is available.
    #[must_use]
    pub fn suggestion(&self) -> Option<&str> {
        self.suggestion.as_deref()
    }
}

/// All semantic compilation errors collected in deterministic order.
#[derive(Clone, Debug, Default, Eq, PartialEq, Error)]
#[error("{}", format_compile_errors(.errors, .metadata))]
pub struct CompileErrors {
    errors: Vec<CompileError>,
    metadata: Vec<DiagnosticMetadata>,
}

impl CompileErrors {
    pub(crate) fn push(
        &mut self,
        error: CompileError,
        labels: Vec<DiagnosticLabel>,
        suggestion: Option<String>,
    ) {
        self.errors.push(error);
        self.metadata
            .push(DiagnosticMetadata { labels, suggestion });
    }

    pub(crate) fn append(&mut self, mut other: Self) {
        self.errors.append(&mut other.errors);
        self.metadata.append(&mut other.metadata);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    /// Returns collected errors.
    #[must_use]
    pub fn errors(&self) -> &[CompileError] {
        &self.errors
    }

    /// Returns source labels and suggestions parallel to [`Self::errors`].
    #[must_use]
    pub fn metadata(&self) -> &[DiagnosticMetadata] {
        &self.metadata
    }

    /// Renders diagnostics using source names supplied by the facade.
    #[must_use]
    pub fn render_with_source_names(&self, sources: &BTreeMap<SourceId, String>) -> String {
        let mut output = String::new();
        self.write_to(&mut output, sources)
            .expect("writing diagnostics to a String cannot fail");
        output
    }

    pub(crate) fn write_to(
        &self,
        output: &mut impl fmt::Write,
        sources: &BTreeMap<SourceId, String>,
    ) -> fmt::Result {
        for (index, (error, metadata)) in self.errors.iter().zip(&self.metadata).enumerate() {
            if index > 0 {
                output.write_str("\n")?;
            }
            writeln!(output, "error[{}]: {error}", error.code())?;
            for label in &metadata.labels {
                let source = sources
                    .get(&label.source)
                    .cloned()
                    .unwrap_or_else(|| format!("source#{}", label.source.index()));
                let location = label.span.map_or_else(
                    || source.clone(),
                    |span| format!("{source}:{}..{}", span.start(), span.end()),
                );
                let kind = match label.kind {
                    LabelKind::Primary => "-->",
                    LabelKind::Secondary => "::: ",
                };
                writeln!(output, "  {kind} {location}: {}", label.message)?;
            }
            if let Some(suggestion) = &metadata.suggestion {
                writeln!(output, "  help: {suggestion}")?;
            }
        }
        Ok(())
    }
}

fn format_compile_errors(errors: &[CompileError], metadata: &[DiagnosticMetadata]) -> String {
    let diagnostics = CompileErrors {
        errors: errors.to_owned(),
        metadata: metadata.to_owned(),
    };
    diagnostics.render_with_source_names(&BTreeMap::new())
}

pub(crate) fn closest_token<'a>(
    needle: &TokenPath,
    candidates: impl IntoIterator<Item = &'a TokenPath>,
) -> Option<&'a TokenPath> {
    closest_text(&needle.to_string(), candidates, |candidate| {
        candidate.to_string()
    })
}

pub(crate) fn closest_name<'a>(
    needle: &Name,
    candidates: impl IntoIterator<Item = &'a Name>,
) -> Option<&'a Name> {
    closest_text(needle.as_str(), candidates, |candidate| {
        candidate.as_str().to_owned()
    })
}

fn closest_text<'a, T>(
    needle: &str,
    candidates: impl IntoIterator<Item = &'a T>,
    text: impl Fn(&T) -> String,
) -> Option<&'a T> {
    let threshold = 3.max(needle.chars().count() / 3);
    candidates
        .into_iter()
        .map(|candidate| {
            let distance = edit_distance(needle, &text(candidate));
            (distance, candidate)
        })
        .filter(|(distance, _)| *distance <= threshold)
        .min_by_key(|(distance, candidate)| (*distance, text(candidate)))
        .map(|(_, candidate)| candidate)
}

fn edit_distance(left: &str, right: &str) -> usize {
    let right: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    for (left_index, left_char) in left.chars().enumerate() {
        let mut current = vec![left_index + 1];
        for (right_index, right_char) in right.iter().enumerate() {
            current.push(
                (previous[right_index + 1] + 1)
                    .min(current[right_index] + 1)
                    .min(previous[right_index] + usize::from(left_char != *right_char)),
            );
        }
        previous = current;
    }
    previous[right.len()]
}
