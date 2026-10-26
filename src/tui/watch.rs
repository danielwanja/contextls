//! Filesystem watching for live refresh: the inspected folder (recursively)
//! plus the directory of every known context file, so edits to
//! `~/.claude/projects/<slug>/memory/MEMORY.md` show up as fast as edits to
//! `./CLAUDE.md`. Events are filtered to files that could matter and handed
//! to the App, which waits for a quiet moment before re-scanning.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use anyhow::{Context, Result};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};

pub struct Watcher {
    _inner: RecommendedWatcher,
    rx: mpsc::Receiver<PathBuf>,
}

/// Directories that are never context, and churn constantly during builds.
const SKIP_DIRS: [&str; 5] = [".git", "target", "node_modules", "dist", ".next"];
/// Extensions a context / memory / rules file can have.
const EXTENSIONS: [&str; 3] = ["md", "mdx", "mdc"];
/// Settings files that change *which* files a tool loads.
const CONFIG_NAMES: [&str; 6] = [
    "settings.json",
    "opencode.json",
    "opencode.jsonc",
    "config.toml",
    ".aider.conf.yml",
    "workspace.json",
];
/// Extension-less rule files; `.roorules-<mode>` matches by prefix.
const DOT_RULE_FILES: [&str; 5] = [
    ".cursorrules",
    ".windsurfrules",
    ".clinerules",
    ".roorules",
    ".rules",
];

impl Watcher {
    /// Watch `root` recursively and each of `dirs` non-recursively.
    /// `known` are the files we already list; changes to them always count.
    pub fn start(root: &Path, dirs: &[PathBuf], known: HashSet<PathBuf>) -> Result<Self> {
        let (tx, rx) = mpsc::channel();
        let mut inner = notify::recommended_watcher(move |res: notify::Result<Event>| {
            let Ok(event) = res else { return };
            if matches!(event.kind, EventKind::Access(_)) {
                return;
            }
            for path in event.paths {
                if relevant(&path, &known) {
                    let _ = tx.send(path);
                }
            }
        })
        .context("creating file watcher")?;
        inner
            .watch(root, RecursiveMode::Recursive)
            .with_context(|| format!("watching {}", root.display()))?;
        for d in dirs {
            if d.starts_with(root) || !d.is_dir() {
                continue;
            }
            // A directory we can't watch (permissions, limits) is not fatal.
            let _ = inner.watch(d, RecursiveMode::NonRecursive);
        }
        Ok(Watcher { _inner: inner, rx })
    }

    /// Changed paths since the last call.
    pub fn drain(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        while let Ok(p) = self.rx.try_recv() {
            out.push(p);
        }
        out
    }
}

/// Is this a path whose change could alter what a tool loads? Config
/// directories like `~/.codex` are busy (caches, logs, sessions), so only
/// files that are context, rules or tool settings count.
fn relevant(path: &Path, known: &HashSet<PathBuf>) -> bool {
    if known.contains(path) {
        return true;
    }
    if path
        .components()
        .any(|c| SKIP_DIRS.contains(&c.as_os_str().to_string_lossy().as_ref()))
    {
        return false;
    }
    let Some(name) = path.file_name().map(|n| n.to_string_lossy()) else {
        return false;
    };
    if CONFIG_NAMES.contains(&name.as_ref()) || DOT_RULE_FILES.iter().any(|d| name.starts_with(d)) {
        return true;
    }
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()),
        // A new directory (e.g. `.claude/rules/`) may hold context; a full
        // re-scan is cheap, so let it through. Dotfiles were handled above.
        None => path.is_dir(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relevance_filter() {
        let known: HashSet<PathBuf> = [PathBuf::from("/h/.claude/projects/x/memory/notes.md")]
            .into_iter()
            .collect();
        for yes in [
            "/r/CLAUDE.md",
            "/r/.cursor/rules/a.mdc",
            "/r/.cursorrules",
            "/r/.roorules-code",
            "/h/.gemini/settings.json",
            "/h/.codex/config.toml",
            "/h/.claude/projects/x/memory/notes.md",
        ] {
            assert!(relevant(Path::new(yes), &known), "{yes} should count");
        }
        for no in [
            "/h/.codex/models_cache.json",
            "/h/.claude/history.jsonl",
            "/r/.DS_Store",
            "/r/target/debug/contextls",
            "/r/node_modules/x/README.md",
            "/r/src/main.rs",
        ] {
            assert!(!relevant(Path::new(no), &known), "{no} should not count");
        }
    }
}
