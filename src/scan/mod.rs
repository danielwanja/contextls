//! Scan driver: resolves every tool, then fills in git status and cross-tool sharing.

pub mod file;
pub mod imports;
pub mod secrets;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};

use crate::git::Git;
use crate::model::Inventory;
use crate::tools::resolve::resolve_tool;
use crate::tools::schema::Definitions;

#[derive(Debug, Clone, Default)]
pub struct ScanOptions {
    /// Tool ids to include; empty = all.
    pub tools: Vec<String>,
    pub git: bool,
}

/// Snapshot of the process environment, so lookups are cheap and testable.
#[derive(Debug, Clone, Default)]
pub struct Env(HashMap<String, String>);

impl Env {
    pub fn from_process() -> Self {
        Env(std::env::vars().collect())
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }
}

pub struct Ctx {
    /// Inspected folder, canonical.
    pub root: PathBuf,
    pub home: PathBuf,
    pub git_root: Option<PathBuf>,
    /// `macos` / `linux` / `windows`.
    pub platform: &'static str,
    pub env: Env,
}

pub fn scan_with(defs: &Definitions, root: &Path, opts: &ScanOptions) -> Result<Inventory> {
    let root = root
        .canonicalize()
        .with_context(|| format!("cannot access {}", root.display()))?;
    let env = Env::from_process();
    let home = env
        .get("HOME")
        .map(PathBuf::from)
        .or_else(|| etcetera::home_dir().ok())
        .context("cannot determine home directory")?;
    // macOS temp dirs and symlinked homes: compare like with like.
    let home = home.canonicalize().unwrap_or(home);
    let git = Git::discover(&root, opts.git);
    let ctx = Ctx {
        root: root.clone(),
        home: home.clone(),
        git_root: git.toplevel.clone(),
        platform: platform(),
        env,
    };

    // Resolve every tool even when filtering, so `shared_with` still sees the others.
    let mut tools: Vec<_> = defs.tools.iter().map(|t| resolve_tool(t, &ctx)).collect();

    // Git status, one batched lookup for every existing file.
    let paths: Vec<PathBuf> = tools
        .iter()
        .flat_map(|t| {
            t.entries
                .iter()
                .filter(|e| e.exists)
                .map(|e| e.path.clone())
        })
        .collect();
    let statuses = git.classify(&paths);
    for e in tools.iter_mut().flat_map(|t| t.entries.iter_mut()) {
        if let Some(s) = statuses.get(&e.path) {
            e.git = *s;
        }
    }

    // Same file (following symlinks) loaded by several tools.
    let mut by_canon: HashMap<PathBuf, Vec<String>> = HashMap::new();
    for t in &tools {
        for e in t.entries.iter().filter(|e| e.exists) {
            let canon = e.path.canonicalize().unwrap_or_else(|_| e.path.clone());
            let ids = by_canon.entry(canon).or_default();
            if !ids.contains(&t.id) {
                ids.push(t.id.clone());
            }
        }
    }
    for t in tools.iter_mut() {
        let id = t.id.clone();
        for e in t.entries.iter_mut().filter(|e| e.exists) {
            let canon = e.path.canonicalize().unwrap_or_else(|_| e.path.clone());
            if let Some(ids) = by_canon.get(&canon) {
                e.shared_with = ids.iter().filter(|i| **i != id).cloned().collect();
            }
        }
    }

    if !opts.tools.is_empty() {
        tools.retain(|t| opts.tools.iter().any(|id| id == &t.id));
    }

    Ok(Inventory {
        root,
        git_root: git.toplevel,
        home,
        tools,
    })
}

fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}

impl Env {
    /// An environment built from explicit pairs (tests).
    pub fn from_pairs(pairs: &[(&str, &str)]) -> Self {
        Env(pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect())
    }
}
