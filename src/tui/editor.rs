//! Suspend the TUI, open a file in `$VISUAL` / `$EDITOR`, resume.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use ratatui::DefaultTerminal;

pub fn open(terminal: &mut DefaultTerminal, path: &Path) -> Result<()> {
    let cmd = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".into());
    let mut parts = cmd.split_whitespace();
    let program = parts.next().unwrap_or("vi");
    let args: Vec<&str> = parts.collect();

    ratatui::restore();
    let status = Command::new(program).args(&args).arg(path).status();
    *terminal = ratatui::init();
    terminal.clear()?;

    status.with_context(|| format!("could not run editor `{program}`"))?;
    Ok(())
}
