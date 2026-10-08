//! Editor integration: the auto-registered plugin and its importer.
//!
//! Editor classes consume [`crate::native`] and [`crate::project`] services and
//! talk to each other through Rust types; only engine signals carry values
//! between them.

pub(crate) mod importer;
pub(crate) mod plugin;
