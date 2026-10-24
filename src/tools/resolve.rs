//! Turns a `ToolDef` plus a scan context into an ordered list of entries.

use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use globset::{GlobBuilder, GlobMatcher};
use ignore::WalkBuilder;
use regex::Regex;
use std::sync::LazyLock;

use crate::model::{Entry, GitStatus, LoadMode, Scope, ToolReport, display_path};
use crate::scan::{Ctx, file, imports, secrets};

use super::schema::{ImportSyntax, LoadModeDef, SourceDef, ToolDef, Walk};
use super::special;

/// Max file size we read for tokens / frontmatter / imports.
const MAX_READ: u64 = 2 * 1024 * 1024;
const DESCENDANT_DEPTH: usize = 10;

pub fn resolve_tool(def: &ToolDef, ctx: &Ctx) -> ToolReport {
    let def = special::adjust(def.clone(), ctx);
    let mut entries = Vec::new();
    for src in &def.sources {
        if src.platform.as_deref().is_some_and(|p| p != ctx.platform) {
            continue;
        }
        for loc in locate(src, &def, ctx) {
            entries.push(entry_for(loc.path, loc.scope, &EntryOpts::from(src), ctx));
        }
    }
    special::finish(&def, ctx, &mut entries);
    dedupe(&mut entries);
    for (i, e) in entries.iter_mut().enumerate() {
        e.order = i + 1;
        e.tool = def.id.clone();
    }
    ToolReport {
        id: def.id,
        name: def.name,
        homepage: def.homepage,
        notes: def.notes,
        entries,
    }
}

/// Everything about how to build an entry that isn't the path itself.
#[derive(Debug, Clone, Default)]
pub struct EntryOpts {
    pub load_mode: LoadModeDef,
    pub frontmatter_globs_key: Option<String>,
    pub import_syntax: ImportSyntax,
    pub label: Option<String>,
    pub legacy: bool,
}

impl From<&SourceDef> for EntryOpts {
    fn from(src: &SourceDef) -> Self {
        EntryOpts {
            load_mode: src.load_mode,
            frontmatter_globs_key: src.frontmatter_globs_key.clone(),
            import_syntax: src.import_syntax,
            label: src.label.clone(),
            legacy: src.legacy,
        }
    }
}

pub struct Located {
    pub path: PathBuf,
    pub scope: Scope,
}

fn locate(src: &SourceDef, def: &ToolDef, ctx: &Ctx) -> Vec<Located> {
    let mut out = Vec::new();
    match src.walk {
        Walk::None => {
            if let Some(p) = &src.path {
                let p = expand(p, def, ctx);
                if src.candidates.is_empty() {
                    out.push(Located {
                        path: p,
                        scope: src.scope,
                    });
                } else {
                    candidates_in_dir(&p, src, &mut out);
                }
            } else if let Some(g) = &src.glob {
                let g = expand(g, def, ctx);
                for path in glob_files(&g.to_string_lossy()) {
                    out.push(Located {
                        path,
                        scope: src.scope,
                    });
                }
            }
        }
        Walk::FirstMatch => {
            for c in &src.candidates {
                let p = ctx.root.join(c);
                if p.is_file() {
                    out.push(Located {
                        path: p,
                        scope: src.scope,
                    });
                    break;
                }
            }
        }
        Walk::Descendants => {
            let matcher = src
                .glob
                .as_ref()
                .and_then(|g| glob_matcher(&format!("**/{g}")));
            for path in walk_descendants(&ctx.root) {
                let Ok(rel) = path.strip_prefix(&ctx.root) else {
                    continue;
                };
                // Root-level files belong to the ancestor-walking sources.
                if rel.components().count() <= 1 {
                    continue;
                }
                if matcher.as_ref().is_some_and(|m| m.is_match(rel)) {
                    out.push(Located {
                        path,
                        scope: src.scope,
                    });
                    continue;
                }
                let name = path.file_name().map(|n| n.to_string_lossy());
                let Some(name) = name else { continue };
                if src.local_variant.as_deref() == Some(name.as_ref()) {
                    out.push(Located {
                        path,
                        scope: Scope::Local,
                    });
                } else if src.candidates.iter().any(|c| c == name.as_ref()) {
                    out.push(Located {
                        path,
                        scope: src.scope,
                    });
                }
            }
        }
        _ => {
            for dir in ancestors(src.walk, ctx) {
                // Per directory: shared files, then rule globs, then the user's local override.
                if !src.candidates.is_empty() {
                    candidates_in_dir(&dir, src, &mut out);
                }
                if let Some(g) = &src.glob {
                    for path in glob_files(&dir.join(g).to_string_lossy()) {
                        out.push(Located {
                            path,
                            scope: src.scope,
                        });
                    }
                }
                local_variant_in_dir(&dir, src, &mut out);
            }
        }
    }
    out
}

fn candidates_in_dir(dir: &Path, src: &SourceDef, out: &mut Vec<Located>) {
    for c in &src.candidates {
        let p = dir.join(c);
        if p.is_file() {
            out.push(Located {
                path: p,
                scope: src.scope,
            });
            if src.one_per_dir {
                break;
            }
        }
    }
    if src.walk == Walk::None {
        local_variant_in_dir(dir, src, out);
    }
}

fn local_variant_in_dir(dir: &Path, src: &SourceDef, out: &mut Vec<Located>) {
    if let Some(lv) = &src.local_variant {
        let p = dir.join(lv);
        if p.is_file() {
            out.push(Located {
                path: p,
                scope: Scope::Local,
            });
        }
    }
}

/// Directories visited for a walking strategy, top-most first (tools concatenate root→cwd).
pub fn ancestors(walk: Walk, ctx: &Ctx) -> Vec<PathBuf> {
    let stop: Option<&Path> = match walk {
        Walk::AncestorsToRoot => None,
        Walk::AncestorsToGitRoot | Walk::GitRootDownToCwd => ctx.git_root.as_deref(),
        Walk::AncestorsToHome => Some(&ctx.home),
        _ => return vec![ctx.root.clone()],
    };
    let mut dirs = Vec::new();
    for dir in ctx.root.ancestors() {
        dirs.push(dir.to_path_buf());
        if Some(dir) == stop {
            break;
        }
    }
    // Not under the stop directory at all (e.g. not inside git): only the folder itself.
    if stop.is_some_and(|s| !ctx.root.starts_with(s)) {
        dirs = vec![ctx.root.clone()];
    }
    dirs.reverse();
    dirs
}

static GLOB_META: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[*?\[\]{}]").unwrap());

fn glob_matcher(pattern: &str) -> Option<GlobMatcher> {
    GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
        .ok()
        .map(|g| g.compile_matcher())
}

/// Files matching an absolute glob pattern, sorted.
fn glob_files(pattern: &str) -> Vec<PathBuf> {
    let pat = Path::new(pattern);
    let mut base = PathBuf::new();
    for comp in pat.components() {
        let s = comp.as_os_str().to_string_lossy();
        if GLOB_META.is_match(&s) {
            break;
        }
        base.push(comp.as_os_str());
    }
    if !base.is_dir() {
        return Vec::new();
    }
    let Some(matcher) = glob_matcher(pattern) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    let walker = WalkBuilder::new(&base)
        .standard_filters(false)
        .hidden(false)
        .sort_by_file_path(|a, b| a.cmp(b))
        .build();
    for dent in walker.flatten() {
        if dent.file_type().is_some_and(|t| t.is_file()) && matcher.is_match(dent.path()) {
            files.push(dent.into_path());
        }
    }
    files
}

/// Files below `root` (not in `root` itself), gitignore-aware, sorted.
fn walk_descendants(root: &Path) -> Vec<PathBuf> {
    let walker = WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .git_exclude(true)
        .require_git(true)
        .max_depth(Some(DESCENDANT_DEPTH))
        .filter_entry(|d| {
            let name = d.file_name().to_string_lossy();
            name != ".git" && name != "node_modules" && name != "target"
        })
        .sort_by_file_path(|a, b| a.cmp(b))
        .build();
    walker
        .flatten()
        .filter(|d| d.file_type().is_some_and(|t| t.is_file()))
        .map(|d| d.into_path())
        .collect()
}

static ENV_VAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$([A-Za-z_][A-Za-z0-9_]*)").unwrap());

/// Expand `{config_dir}`, `$VAR` and a leading `~`.
pub fn expand(s: &str, def: &ToolDef, ctx: &Ctx) -> PathBuf {
    let mut s = s.to_string();
    if s.contains("{config_dir}") {
        let dir = config_dir(def, ctx)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        s = s.replace("{config_dir}", &dir);
    }
    s = ENV_VAR
        .replace_all(&s, |caps: &regex::Captures| {
            ctx.env.get(&caps[1]).unwrap_or_default()
        })
        .into_owned();
    expand_tilde(&s, &ctx.home)
}

pub fn expand_tilde(s: &str, home: &Path) -> PathBuf {
    if s == "~" {
        return home.to_path_buf();
    }
    if let Some(rest) = s.strip_prefix("~/") {
        return home.join(rest);
    }
    PathBuf::from(s)
}

/// The tool's per-user config directory, honouring its env override.
pub fn config_dir(def: &ToolDef, ctx: &Ctx) -> Option<PathBuf> {
    let cd = def.config_dir.as_ref()?;
    if let Some(v) = cd
        .env
        .as_deref()
        .and_then(|var| ctx.env.get(var))
        .filter(|v| !v.is_empty())
    {
        return Some(expand_tilde(&v, &ctx.home));
    }
    Some(expand_tilde(&cd.default, &ctx.home))
}

/// Build a fully populated entry for `path` (git status is filled in later by the scan driver).
pub fn entry_for(path: PathBuf, scope: Scope, opts: &EntryOpts, ctx: &Ctx) -> Entry {
    let path = lexical_normalize(&path);
    let meta = fs::metadata(&path).ok();
    let exists = meta.as_ref().is_some_and(|m| m.is_file());
    let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
    let mtime = meta.as_ref().and_then(|m| m.modified().ok());
    let content = if exists && size <= MAX_READ {
        fs::read_to_string(&path).ok()
    } else {
        None
    };

    let mut warnings = Vec::new();
    let est_tokens = content
        .as_deref()
        .map(|c| (c.chars().count() as u64).div_ceil(4))
        .unwrap_or(0);
    let frontmatter = match content.as_deref().map(file::frontmatter) {
        Some(Ok(fm)) => fm,
        Some(Err(e)) => {
            warnings.push(format!("frontmatter: {e}"));
            None
        }
        None => None,
    };
    let imports = match (opts.import_syntax, content.as_deref()) {
        (ImportSyntax::AtPath, Some(c)) => imports::at_path(c, &path, &ctx.home),
        (ImportSyntax::KiroRef, Some(c)) => imports::kiro_ref(c, &path, &ctx.root),
        _ => Vec::new(),
    };
    if let Some(c) = content.as_deref() {
        warnings.extend(secrets::scan(c));
    }
    for imp in &imports {
        if !imp.exists {
            warnings.push(format!(
                "import `{}` not found (line {})",
                imp.raw, imp.line
            ));
        }
    }
    let load_mode = derive_load_mode(opts, frontmatter.as_ref());
    let display = display_path(&path, &ctx.root, &ctx.home);

    Entry {
        tool: String::new(),
        scope,
        order: 0,
        path,
        display_path: display,
        exists,
        size,
        est_tokens,
        mtime,
        git: GitStatus::Unknown,
        load_mode,
        legacy: opts.legacy,
        frontmatter,
        imports,
        shared_with: Vec::new(),
        warnings,
        label: opts.label.clone(),
    }
}

fn derive_load_mode(opts: &EntryOpts, fm: Option<&serde_json::Value>) -> LoadMode {
    match opts.load_mode {
        LoadModeDef::Always => LoadMode::Always,
        LoadModeDef::OnDemand => LoadMode::OnDemand,
        LoadModeDef::Manual => LoadMode::Manual,
        LoadModeDef::Fallback => LoadMode::Fallback,
        LoadModeDef::Frontmatter => {
            let Some(fm) = fm else {
                return LoadMode::Always;
            };
            if fm.get("alwaysApply").and_then(|v| v.as_bool()) == Some(true) {
                return LoadMode::Always;
            }
            // Windsurf `trigger`, Kiro `inclusion`.
            let mode = fm
                .get("trigger")
                .or_else(|| fm.get("inclusion"))
                .and_then(|v| v.as_str());
            match mode {
                Some("always_on" | "always") => return LoadMode::Always,
                Some("manual") => return LoadMode::Manual,
                Some("model_decision" | "auto") => return LoadMode::Intelligent,
                _ => {}
            }
            let globs = opts
                .frontmatter_globs_key
                .as_deref()
                .and_then(|k| fm.get(k))
                .map(file::string_list)
                .unwrap_or_default();
            if !globs.is_empty() {
                LoadMode::Conditional { globs }
            } else {
                LoadMode::Always
            }
        }
    }
}

/// Collapse `.` and `..` without touching the filesystem.
pub fn lexical_normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn dedupe(entries: &mut Vec<Entry>) {
    let mut seen = HashSet::new();
    entries.retain(|e| seen.insert(e.path.clone()));
}
