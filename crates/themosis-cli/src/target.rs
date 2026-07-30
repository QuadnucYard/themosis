//! Feature-aware backend target selection and command dispatch.

use std::{path::Path, process::ExitCode};

use clap::{Args, ValueEnum};
use themosis_core::CompiledTheme;

/// A backend supported by targeted CLI commands.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum Target {
    /// Generate or validate a native Godot theme.
    #[cfg(feature = "godot")]
    Godot,
}

/// Backend-specific arguments shared by targeted CLI commands.
#[derive(Debug, Args)]
pub(crate) struct TargetOptions {
    #[cfg(feature = "godot")]
    #[command(flatten)]
    godot: crate::godot::RuntimeOptions,
}

impl Target {
    /// Builds this target's artifact from a compiled theme.
    #[cfg_attr(not(feature = "godot"), allow(unused_variables))]
    pub(crate) fn build(
        self,
        root: &Path,
        output: &Path,
        theme: &CompiledTheme,
        options: &TargetOptions,
    ) -> ExitCode {
        match self {
            #[cfg(feature = "godot")]
            Self::Godot => crate::godot::build_theme(root, output, theme, &options.godot),
        }
    }

    /// Validates this target against a compiled theme.
    #[cfg_attr(not(feature = "godot"), allow(unused_variables))]
    pub(crate) fn check(
        self,
        root: &Path,
        theme: &CompiledTheme,
        options: &TargetOptions,
    ) -> ExitCode {
        match self {
            #[cfg(feature = "godot")]
            Self::Godot => crate::godot::check_theme(root, theme, &options.godot),
        }
    }
}
