use std::fs;
use std::path::Path;

use crate::scan::{Ctx, file};
use crate::tools::resolve::{config_dir, expand_tilde};
use crate::tools::schema::{ToolDef, Walk};

/// Honour `context.fileName` from the user and workspace settings.json.
pub fn adjust(mut def: ToolDef, ctx: &Ctx) -> ToolDef {
    let mut names: Option<Vec<String>> = None;
    let mut candidates = Vec::new();
    if let Some(cfg) = config_dir(&def, ctx) {
        candidates.push(cfg.join("settings.json"));
    }
    candidates.push(ctx.root.join(".gemini").join("settings.json"));
    if let Some(sys) = ctx.env.get("GEMINI_CLI_SYSTEM_SETTINGS_PATH") {
        candidates.insert(0, expand_tilde(&sys, &ctx.home));
    }
    // Later files win, so the workspace file overrides the user file.
    for path in &candidates {
        if let Some(n) = file_names_from(path) {
            names = Some(n);
        }
    }
    let Some(names) = names.filter(|n| !n.is_empty()) else {
        return def;
    };

    for src in &mut def.sources {
        match src.walk {
            Walk::None if src.path.as_deref() == Some("{config_dir}/GEMINI.md") => {
                if names.len() == 1 {
                    src.path = Some(format!("{{config_dir}}/{}", names[0]));
                } else {
                    src.path = Some("{config_dir}".into());
                    src.candidates = names.clone();
                }
            }
            _ if src.candidates == ["GEMINI.md"] => src.candidates = names.clone(),
            _ => {}
        }
    }
    def.notes
        .push(format!("context.fileName is set: {}", names.join(", ")));
    def
}

fn file_names_from(path: &Path) -> Option<Vec<String>> {
    let text = fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let name = v.get("context")?.get("fileName")?;
    Some(file::string_list(name))
}
