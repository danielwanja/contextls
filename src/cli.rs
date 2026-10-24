use std::path::PathBuf;

use clap::{Parser, ValueEnum};

/// contextls — see which context / memory / instruction files each AI coding
/// tool loads for a folder, in load order.
///
/// Runs an interactive TUI when stdout is a terminal; otherwise (or with
/// --plain / --json) prints a table or JSON. Read-only, never touches the
/// network.
#[derive(Debug, Parser)]
#[command(name = "contextls", version, about, long_about = None)]
pub struct Args {
    /// Folder to inspect (defaults to the current directory)
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,

    /// Print a plain table instead of the TUI
    #[arg(long, conflicts_with = "json")]
    pub plain: bool,

    /// Print JSON instead of the TUI
    #[arg(long)]
    pub json: bool,

    /// Only show these tools (repeatable; see --list-tools for ids)
    #[arg(short = 't', long = "tool", value_name = "ID")]
    pub tools: Vec<String>,

    /// List the known tool ids and exit
    #[arg(long)]
    pub list_tools: bool,

    /// Include expected-but-missing locations in plain/JSON output
    #[arg(short = 'a', long)]
    pub all: bool,

    /// Show tools not detected on this machine like any other, instead of
    /// collapsing them under "Not detected"
    #[arg(long)]
    pub all_tools: bool,

    /// Skip git lookups (tracked / ignored status, git root detection)
    #[arg(long)]
    pub no_git: bool,

    /// Group the TUI tree by scope first instead of by tool
    #[arg(long, value_enum, default_value_t = Grouping::Tool)]
    pub group: Grouping,

    /// AI CLI to use for on-demand summaries (`s` in the TUI); defaults to the
    /// first installed of claude, codex, grok, … (see --list-summarizers)
    #[arg(long, value_name = "ID")]
    pub summarizer: Option<String>,

    /// List the summarizer CLIs contextls knows, marking installed ones, and exit
    #[arg(long)]
    pub list_summarizers: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Grouping {
    Tool,
    Scope,
}
