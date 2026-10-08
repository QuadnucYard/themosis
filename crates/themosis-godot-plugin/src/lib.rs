//! Native Godot resource construction and GDExtension integration for Themosis.
//!
//! Compilation and the engine-facing classes live in Rust modules; the
//! engine-facing classes are thin: they own nodes, run engine virtuals, and
//! forward events. No Godot dictionary crosses an internal boundary.
//!
//! - [`native`]: engine-side services — native theme construction, generation,
//!   persistence, diagnostics, and the imported-artifact cache contract.
//! - [`project`]: project state — source discovery and the `res://` source
//!   provider.
//! - [`editor`]: the auto-registered editor plugin and its importer.
//! - [`runners`]: native main loops for the CLI protocol and the import gate.
//!
//! The dependency rule is one-way: `editor` and `runners` use `native` and
//! `project` services; those service layers never depend on editor classes.

use godot::prelude::*;

mod editor;
mod native;
mod project;
mod runners;

#[cfg(feature = "test-support")]
mod test_support;

struct ThemosisExtension;

#[gdextension]
unsafe impl ExtensionLibrary for ThemosisExtension {}
