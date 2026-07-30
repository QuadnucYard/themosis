//! Themosis command-line interface.

#![forbid(unsafe_code)]

mod cli;
mod commands;
#[cfg(feature = "godot")]
pub(crate) mod godot;
pub(crate) mod source;
mod target;

fn main() -> std::process::ExitCode {
    cli::run()
}
