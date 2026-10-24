//! Core data model shared by the scanner, the plain outputs and the TUI.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Serialize;

/// Everything we found for one folder.
#[derive(Debug, Clone, Serialize)]
pub struct Inventory {
    /// The folder that was inspected (canonical).
    pub root: PathBuf,
    /// Git toplevel containing `root`, if any.
    pub git_root: Option<PathBuf>,
    pub home: PathBuf,
    pub tools: Vec<ToolReport>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolReport {
    pub id: String,
    pub name: String,
    pub homepage: String,
    /// Short caveats shown in the details pane (e.g. "User rules live in the settings DB").
    pub notes: Vec<String>,
    /// Whether the tool seems to be installed on this machine.
    pub install: Install,
    /// In load order.
    pub entries: Vec<Entry>,
}

impl ToolReport {
    pub fn present(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter().filter(|e| e.exists)
    }

    pub fn total_tokens(&self) -> u64 {
        self.present().map(|e| e.est_tokens).sum()
    }

    /// Shown as a regular tool: installed, or of unknown state but loading something here.
    /// Everything else is collapsed under "Not detected".
    pub fn active(&self) -> bool {
        match &self.install {
            Install::Installed { .. } => true,
            Install::NotInstalled { .. } => false,
            Install::Unknown => self.present().next().is_some(),
        }
    }
}

/// Evidence-based install state, from the tool's `detect` rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Install {
    Installed {
        evidence: String,
    },
    NotInstalled {
        looked_for: Vec<String>,
    },
    /// The tool has no detection rules.
    Unknown,
}

impl Install {
    pub fn summary(&self) -> String {
        match self {
            Install::Installed { evidence } => format!("installed · {evidence}"),
            Install::NotInstalled { looked_for } => {
                format!("not detected · looked for {}", looked_for.join(", "))
            }
            Install::Unknown => "install state unknown".into(),
        }
    }
}

/// Where a file sits in the tool's precedence ladder.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// Machine-wide / enterprise managed file.
    Managed,
    /// Per-user (home directory) file.
    User,
    /// Checked-in project file.
    Project,
    /// Per-user override inside the project (e.g. CLAUDE.local.md).
    Local,
    /// Tool-written memory the user did not author directly.
    AutoMemory,
}

impl Scope {
    pub fn label(self) -> &'static str {
        match self {
            Scope::Managed => "Managed",
            Scope::User => "User",
            Scope::Project => "Project",
            Scope::Local => "Local",
            Scope::AutoMemory => "Auto memory",
        }
    }
}

/// When the tool actually puts the file into context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LoadMode {
    /// Loaded at session start.
    Always,
    /// Loaded when the agent touches a file in that subtree.
    OnDemand,
    /// Loaded when a touched file matches one of these globs.
    Conditional { globs: Vec<String> },
    /// Only when the user @-mentions it.
    Manual,
    /// Only used if nothing earlier in the ladder exists.
    Fallback,
    /// The tool decides from the description whether to include it.
    Intelligent,
}

impl LoadMode {
    pub fn badge(&self) -> Option<String> {
        match self {
            LoadMode::Always => None,
            LoadMode::OnDemand => Some("on-demand".into()),
            LoadMode::Conditional { globs } => Some(format!("glob:{}", globs.join(","))),
            LoadMode::Manual => Some("manual".into()),
            LoadMode::Fallback => Some("fallback".into()),
            LoadMode::Intelligent => Some("auto".into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GitStatus {
    Tracked,
    Untracked,
    Ignored,
    /// Path is not inside the repository (or there is no repository).
    NotRepo,
    /// git unavailable or lookup skipped.
    Unknown,
}

impl GitStatus {
    pub fn label(self) -> &'static str {
        match self {
            GitStatus::Tracked => "tracked",
            GitStatus::Untracked => "untracked",
            GitStatus::Ignored => "ignored",
            GitStatus::NotRepo => "-",
            GitStatus::Unknown => "?",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Import {
    /// Text as written in the file (e.g. `@docs/style.md`).
    pub raw: String,
    pub resolved: PathBuf,
    pub exists: bool,
    /// 1-based line number.
    pub line: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    pub tool: String,
    pub scope: Scope,
    /// 1-based position in this tool's load order.
    pub order: usize,
    pub path: PathBuf,
    /// `~`-abbreviated or project-relative path for display.
    pub display_path: String,
    pub exists: bool,
    pub size: u64,
    /// chars / 4 — a rough estimate, always shown with "≈".
    pub est_tokens: u64,
    #[serde(serialize_with = "ser_mtime")]
    pub mtime: Option<SystemTime>,
    pub git: GitStatus,
    pub load_mode: LoadMode,
    /// Deprecated location the tool still honours.
    pub legacy: bool,
    /// Parsed YAML frontmatter, if any.
    pub frontmatter: Option<serde_json::Value>,
    pub imports: Vec<Import>,
    /// Other tool ids that load this same file (canonical path match).
    pub shared_with: Vec<String>,
    pub warnings: Vec<String>,
    /// Optional human label (e.g. "rules", "steering", "prompts").
    pub label: Option<String>,
}

impl Entry {
    pub fn file_name(&self) -> String {
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.display().to_string())
    }
}

fn ser_mtime<S: serde::Serializer>(t: &Option<SystemTime>, s: S) -> Result<S::Ok, S::Error> {
    match t {
        Some(t) => s.serialize_some(&humantime::format_rfc3339_seconds(*t).to_string()),
        None => s.serialize_none(),
    }
}

/// `~`-abbreviate paths under home, otherwise make them relative to `root` when inside it.
pub fn display_path(path: &Path, root: &Path, home: &Path) -> String {
    if let Ok(rel) = path.strip_prefix(root) {
        let s = rel.to_string_lossy();
        return if s.is_empty() {
            ".".into()
        } else {
            format!("./{s}")
        };
    }
    if let Ok(rel) = path.strip_prefix(home) {
        return format!("~/{}", rel.to_string_lossy());
    }
    path.to_string_lossy().into_owned()
}
