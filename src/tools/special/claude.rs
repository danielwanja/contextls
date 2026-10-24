use std::fs;

use crate::model::{Entry, Scope};
use crate::scan::Ctx;
use crate::tools::resolve::{EntryOpts, ancestors, config_dir, entry_for};
use crate::tools::schema::{ImportSyntax, LoadModeDef, ToolDef, Walk};

pub fn finish(def: &ToolDef, ctx: &Ctx, entries: &mut Vec<Entry>) {
    agents_md_fallback(ctx, entries);
    auto_memory(def, ctx, entries);
}

/// AGENTS.md is consulted only when no CLAUDE.md exists in the folder or any ancestor.
fn agents_md_fallback(ctx: &Ctx, entries: &mut Vec<Entry>) {
    let has_claude_md = entries.iter().any(|e| {
        e.exists
            && matches!(e.scope, Scope::Project | Scope::Local)
            && matches!(e.file_name().as_str(), "CLAUDE.md" | "CLAUDE.local.md")
    });
    if has_claude_md {
        return;
    }
    let opts = EntryOpts {
        load_mode: LoadModeDef::Fallback,
        import_syntax: ImportSyntax::AtPath,
        ..Default::default()
    };
    for dir in ancestors(Walk::AncestorsToRoot, ctx) {
        for name in ["AGENTS.md", ".claude/AGENTS.md"] {
            let p = dir.join(name);
            if p.is_file() {
                entries.push(entry_for(p, Scope::Project, &opts, ctx));
            }
        }
    }
}

/// `~/.claude/projects/<slug>/memory/` — the slug is the project path with
/// every non-alphanumeric character replaced by `-`.
fn auto_memory(def: &ToolDef, ctx: &Ctx, entries: &mut Vec<Entry>) {
    if ctx.env.get("CLAUDE_CODE_DISABLE_AUTO_MEMORY").as_deref() == Some("1") {
        return;
    }
    let Some(cfg) = config_dir(def, ctx) else {
        return;
    };
    let base = ctx.git_root.as_deref().unwrap_or(&ctx.root);
    let slug = ctx
        .env
        .get("CLAUDE_CODE_PROJECT_DIR_NAME")
        .unwrap_or_else(|| project_slug(&base.to_string_lossy()));
    let dir = cfg.join("projects").join(slug).join("memory");
    let opts = EntryOpts {
        label: Some("memory".into()),
        ..Default::default()
    };
    entries.push(entry_for(
        dir.join("MEMORY.md"),
        Scope::AutoMemory,
        &opts,
        ctx,
    ));
    let Ok(rd) = fs::read_dir(&dir) else { return };
    let mut others: Vec<_> = rd
        .flatten()
        .map(|d| d.path())
        .filter(|p| {
            p.is_file()
                && p.extension().is_some_and(|e| e == "md")
                && p.file_name().is_some_and(|n| n != "MEMORY.md")
        })
        .collect();
    others.sort();
    for p in others {
        entries.push(entry_for(p, Scope::AutoMemory, &opts, ctx));
    }
}

pub fn project_slug(path: &str) -> String {
    path.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn slug_replaces_non_alphanumerics() {
        assert_eq!(
            super::project_slug("/Users/dan/GitProjects/my.repo"),
            "-Users-dan-GitProjects-my-repo"
        );
    }
}
