//! Project-aware Godot commands backed by the native runner.

mod command;
mod output;
mod runtime;

pub(crate) use command::Godot;
