//! Serde mirror of `tools/definitions.toml`.
//!
//! Everything a tool's lookup rules can express declaratively lives here; the
//! handful of behaviours TOML can't describe (settings files, auto-memory
//! slugs, first-match ladders) are in `super::special`.

use serde::Deserialize;

use crate::model::Scope;

#[derive(Debug, Clone, Deserialize)]
pub struct Definitions {
    #[serde(rename = "tool")]
    pub tools: Vec<ToolDef>,
    /// AI CLIs usable for on-demand summaries, in preference order.
    #[serde(rename = "summarizer", default)]
    pub summarizers: Vec<crate::summarize::SummarizerDef>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolDef {
    pub id: String,
    pub name: String,
    pub homepage: String,
    #[serde(default)]
    pub notes: Vec<String>,
    /// Per-user config directory; `{config_dir}` in paths expands to it.
    pub config_dir: Option<ConfigDir>,
    /// How to tell the tool is installed here. Absent = unknown.
    pub detect: Option<Detect>,
    #[serde(rename = "source", default)]
    pub sources: Vec<SourceDef>,
}

/// Any one match means "installed". Paths support `~`, `$VAR`, `{config_dir}`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Detect {
    /// Directories or files whose existence proves installation.
    #[serde(default)]
    pub dirs: Vec<String>,
    /// Executables looked up on PATH.
    #[serde(default)]
    pub bins: Vec<String>,
    /// macOS app bundles checked in /Applications and ~/Applications.
    #[serde(default)]
    pub apps: Vec<String>,
    /// Globs (files or directories), e.g. an editor extension folder.
    #[serde(default)]
    pub globs: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigDir {
    /// May start with `~`.
    pub default: String,
    /// Environment variable that replaces the whole directory.
    pub env: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceDef {
    pub scope: Scope,

    /// One fixed file. Supports `~`, `$VAR`, `{config_dir}`.
    pub path: Option<String>,
    /// File names tried in each visited directory (with `walk`).
    #[serde(default)]
    pub candidates: Vec<String>,
    /// Glob relative to each visited directory (or absolute / `~` / `{config_dir}` when `walk = none`).
    pub glob: Option<String>,

    #[serde(default)]
    pub walk: Walk,
    /// With `candidates`: stop at the first name that exists in a directory.
    #[serde(default)]
    pub one_per_dir: bool,
    /// Per-user override file name loaded right after each candidate match.
    pub local_variant: Option<String>,

    #[serde(default)]
    pub load_mode: LoadModeDef,
    #[serde(default)]
    pub import_syntax: ImportSyntax,
    /// Frontmatter key holding activation globs (`globs`, `paths`, `applyTo`, `fileMatchPattern`).
    pub frontmatter_globs_key: Option<String>,

    /// Only on this OS (`macos`, `linux`, `windows`).
    pub platform: Option<String>,
    #[serde(default)]
    pub legacy: bool,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Walk {
    /// `path` / `glob` are resolved once, as given.
    #[default]
    None,
    /// Inspected folder and every parent up to `/`, listed root-first.
    AncestorsToRoot,
    /// Inspected folder and parents up to the git toplevel (or just the folder when not in git).
    AncestorsToGitRoot,
    /// Inspected folder and parents up to `$HOME`.
    AncestorsToHome,
    /// Git toplevel down to the inspected folder (same set as `ancestors_to_git_root`, named for the tool's own wording).
    GitRootDownToCwd,
    /// Subdirectories of the inspected folder (files there load on demand).
    Descendants,
    /// Only the inspected folder; first existing candidate wins.
    FirstMatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadModeDef {
    #[default]
    Always,
    OnDemand,
    Manual,
    Fallback,
    /// Derived from frontmatter (`alwaysApply`, `trigger`, `inclusion`, globs key…).
    Frontmatter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportSyntax {
    #[default]
    None,
    /// `@path`, `@./path`, `@~/path`, `@/abs` (Claude Code, Gemini CLI, Amp).
    AtPath,
    /// `#[[file:path]]` (Kiro).
    KiroRef,
}
