//! Native Godot main loops replacing the addon's former scripts.
//!
//! The CLI runner speaks the `themosis-cli` request/response protocol; the
//! build runner materializes profiles, and the check runner validates that
//! every discovered root still compiles and that its imported resource is
//! current.

pub(crate) mod build;
pub(crate) mod check;
pub(crate) mod cli;
