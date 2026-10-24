//! contextls — see which context / memory / instruction files each AI coding
//! tool loads for a folder. The binary in `main.rs` is a thin wrapper; the
//! library exists so the TUI can be rendered in tests.

pub mod cli;
pub mod git;
pub mod model;
pub mod output;
pub mod scan;
pub mod summarize;
pub mod tools;
pub mod tui;
