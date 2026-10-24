use std::fs;

use crate::model::{Entry, Scope};
use crate::scan::Ctx;
use crate::tools::resolve::config_dir;
use crate::tools::schema::{ToolDef, Walk};

const DEFAULT_MAX_BYTES: u64 = 32 * 1024;

fn config(def: &ToolDef, ctx: &Ctx) -> Option<toml::Value> {
    let path = config_dir(def, ctx)?.join("config.toml");
    let text = fs::read_to_string(path).ok()?;
    toml::from_str(&text).ok()
}

/// Append `project_doc_fallback_filenames` to the per-directory candidate list.
pub fn adjust(mut def: ToolDef, ctx: &Ctx) -> ToolDef {
    let Some(cfg) = config(&def, ctx) else {
        return def;
    };
    let fallbacks: Vec<String> = cfg
        .get("project_doc_fallback_filenames")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    if fallbacks.is_empty() {
        return def;
    }
    for src in &mut def.sources {
        if src.walk == Walk::GitRootDownToCwd {
            src.candidates.extend(fallbacks.iter().cloned());
        }
    }
    def.notes.push(format!(
        "project_doc_fallback_filenames: {}",
        fallbacks.join(", ")
    ));
    def
}

/// Warn when the concatenated project docs exceed `project_doc_max_bytes`.
pub fn finish(def: &ToolDef, ctx: &Ctx, entries: &mut [Entry]) {
    let max = config(def, ctx)
        .and_then(|c| c.get("project_doc_max_bytes")?.as_integer())
        .map(|n| n as u64)
        .unwrap_or(DEFAULT_MAX_BYTES);
    let total: u64 = entries
        .iter()
        .filter(|e| e.exists && e.scope == Scope::Project)
        .map(|e| e.size)
        .sum();
    if total > max
        && let Some(last) = entries
            .iter_mut()
            .rev()
            .find(|e| e.exists && e.scope == Scope::Project)
    {
        last.warnings.push(format!(
            "project docs total {total} bytes > project_doc_max_bytes {max}; Codex truncates"
        ));
    }
}
