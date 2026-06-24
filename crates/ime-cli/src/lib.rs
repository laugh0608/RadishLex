//! CLI helpers and demo adapter for RadishLex.

#![forbid(unsafe_code)]

mod cli;
mod demo_engine;

pub use cli::{run, CliError};
pub use demo_engine::DemoEngine;
