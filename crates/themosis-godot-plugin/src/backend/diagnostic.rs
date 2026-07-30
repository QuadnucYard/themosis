use themosis_core::{Diagnostic, Errors};
use themosis_godot::BackendErrors;
use thiserror::Error;

/// One diagnostic returned by the engine-native theme builder.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
#[error(
    "{}",
    format_native_diagnostic(.style, .target, .state, .property, .message)
)]
pub struct NativeDiagnostic {
    pub(super) code: String,
    pub(super) message: String,
    pub(super) style: String,
    pub(super) target: String,
    pub(super) state: String,
    pub(super) property: String,
}

impl NativeDiagnostic {
    /// Returns the stable diagnostic code.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Returns the diagnostic message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the compiled style name, when the diagnostic concerns a style.
    #[must_use]
    pub fn style(&self) -> &str {
        &self.style
    }

    /// Returns the native control target, when one was available.
    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Returns the source state, when the item came from a state block.
    #[must_use]
    pub fn state(&self) -> &str {
        &self.state
    }

    /// Returns the native theme-item name, when one was available.
    #[must_use]
    pub fn property(&self) -> &str {
        &self.property
    }
}

impl Diagnostic for NativeDiagnostic {
    fn code(&self) -> &str {
        &self.code
    }
}

fn format_native_diagnostic(
    style: &str,
    target: &str,
    state: &str,
    property: &str,
    message: &str,
) -> String {
    let context = [
        ("style", style),
        ("target", target),
        ("state", state),
        ("property", property),
    ]
    .into_iter()
    .filter(|(_, value)| !value.is_empty())
    .map(|(field, value)| format!("{field}={value}"))
    .collect::<Vec<_>>()
    .join(" ");
    if context.is_empty() {
        message.to_owned()
    } else {
        format!("[{context}] {message}")
    }
}

/// Engine-native diagnostics collected in builder response order.
pub type NativeDiagnostics = Errors<NativeDiagnostic>;

/// Failure while preparing or constructing a native Godot theme.
#[derive(Debug, Error)]
pub enum ThemeBuildError {
    /// Portable mapping preparation failed before entering Godot.
    #[error("{0}")]
    Preparation(#[from] BackendErrors),
    /// The running engine rejected one or more native mappings.
    #[error("{0}")]
    Native(#[source] NativeDiagnostics),
    /// The embedded builder could not be executed or returned malformed data.
    #[error("native Godot builder failed: {0}")]
    Builder(String),
}

impl ThemeBuildError {
    /// Returns native runtime diagnostics, when the builder reached mapping.
    #[must_use]
    pub fn native_diagnostics(&self) -> Option<&[NativeDiagnostic]> {
        match self {
            Self::Native(diagnostics) => Some(diagnostics.errors()),
            Self::Preparation(_) | Self::Builder(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use super::{NativeDiagnostic, NativeDiagnostics, ThemeBuildError};

    #[test]
    fn formats_native_diagnostics_with_dynamic_codes() {
        let diagnostics = NativeDiagnostics::new(vec![
            NativeDiagnostic {
                code: "unsupported_property".to_owned(),
                message: "property is unavailable".to_owned(),
                style: "Probe".to_owned(),
                target: "Button".to_owned(),
                state: String::new(),
                property: "missing".to_owned(),
            },
            NativeDiagnostic {
                code: "invalid_plan".to_owned(),
                message: "plan is invalid".to_owned(),
                style: String::new(),
                target: String::new(),
                state: String::new(),
                property: String::new(),
            },
        ]);

        assert_eq!(
            diagnostics.to_string(),
            "error[unsupported_property]: [style=Probe target=Button property=missing] property is unavailable\nerror[invalid_plan]: plan is invalid"
        );

        let error = ThemeBuildError::Native(diagnostics);
        assert!(error.source().is_some());
        assert!(
            error
                .to_string()
                .starts_with("error[unsupported_property]:")
        );
    }
}
