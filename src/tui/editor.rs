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

/// Reveal `path` in the system file manager (Finder on macOS), without
/// blocking the TUI. A missing file reveals its nearest existing ancestor.
pub fn reveal(path: &Path) -> Result<()> {
    let target = path
        .ancestors()
        .find(|p| p.exists())
        .unwrap_or(path)
        .to_path_buf();

    let mut cmd = if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        if target.is_file() {
            c.arg("-R");
        }
        c.arg(&target);
        c
    } else if cfg!(target_os = "windows") {
        let mut c = Command::new("explorer");
        if target.is_file() {
            c.arg(format!("/select,{}", target.display()));
        } else {
            c.arg(&target);
        }
        c
    } else {
        let dir = if target.is_file() {
            target.parent().unwrap_or(&target).to_path_buf()
        } else {
            target.clone()
        };
        let mut c = Command::new("xdg-open");
        c.arg(dir);
        c
    };
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .with_context(|| format!("could not open a file manager for {}", target.display()))?;
    Ok(())
}
