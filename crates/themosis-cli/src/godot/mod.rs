//! Project-aware Godot commands backed by the native runner.

mod command;
mod output;
mod runtime;
mod source;

pub(crate) use command::Godot;
