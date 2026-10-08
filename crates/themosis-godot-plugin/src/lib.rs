//! Native Godot resource construction and GDExtension integration for Themosis.
//!
//! Compilation and the engine-facing classes live in Rust modules; the
//! engine-facing classes are thin: they own nodes, run engine virtuals, and
//! forward events. No Godot dictionary crosses an internal boundary.
//!
//! - [`native`]: engine-side services — native theme construction, generation,
//!   persistence, and diagnostics.
//! - [`project`]: project state — the `res://` source provider.
//! - [`runners`]: native main loops for the CLI protocol.
//!
//! The dependency rule is one-way: `runners` use `native` and `project`
//! services; those service layers never depend on engine-facing classes.

use godot::prelude::*;

mod native;
mod project;
mod runners;

#[cfg(feature = "test-support")]
mod test_support;

struct ThemosisExtension;

#[gdextension]
unsafe impl ExtensionLibrary for ThemosisExtension {}
