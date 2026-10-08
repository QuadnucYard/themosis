//! Editor integration: the auto-registered plugin, its importer, and its dock.
//!
//! Editor classes consume [`crate::native`] and [`crate::project`] services and
//! talk to each other through Rust types; only engine signals carry values
//! between them.

pub(crate) mod dock;
pub(crate) mod importer;
pub(crate) mod plugin;
