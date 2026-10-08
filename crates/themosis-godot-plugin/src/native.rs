//! Engine-side native services shared by the editor classes and the runners.
//!
//! This layer knows Godot and the canonical compiler output, but nothing about
//! the editor: [`crate::runners`] depends on these services, never the other
//! way around.

pub(crate) mod backend;
pub(crate) mod diagnostics;
// The importer and profile builds are this service's first non-test
// consumers; until they land, only the native probes call it.
#[allow(dead_code)]
pub(crate) mod generation;
pub(crate) mod materialize;
