//! Per-tool behaviour that `definitions.toml` can't express.
//!
//! `adjust` runs before resolution (rewrite sources from the tool's own
//! settings), `finish` after (append derived entries, refine load modes).

mod aider;
mod claude;
mod codex;
mod copilot;
mod cursor;
mod gemini;

use crate::model::Entry;
use crate::scan::Ctx;

use super::schema::ToolDef;

pub fn adjust(def: ToolDef, ctx: &Ctx) -> ToolDef {
    match def.id.as_str() {
        "gemini-cli" => gemini::adjust(def, ctx),
        "codex-cli" => codex::adjust(def, ctx),
        _ => def,
    }
}

pub fn finish(def: &ToolDef, ctx: &Ctx, entries: &mut Vec<Entry>) {
    match def.id.as_str() {
        "aider" => aider::finish(def, ctx, entries),
        "claude-code" => claude::finish(def, ctx, entries),
        "codex-cli" => codex::finish(def, ctx, entries),
        "copilot" => copilot::finish(def, ctx, entries),
        "cursor" => cursor::finish(entries),
        _ => {}
    }
}
