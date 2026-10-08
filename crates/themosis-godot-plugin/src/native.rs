//! Engine-side native services shared by the editor classes and the runners.
//!
//! This layer knows Godot and the canonical compiler output, but nothing about
//! the editor: [`crate::editor`] and [`crate::runners`] depend on these
//! services, never the other way around.

pub(crate) mod backend;
pub(crate) mod diagnostics;
pub(crate) mod generation;
pub(crate) mod import_cache;
pub(crate) mod materialize;
