use std::fs;
use std::path::Path;

use crate::model::Entry;
use crate::scan::{Ctx, file};
use crate::tools::resolve::{EntryOpts, entry_for, expand_tilde};
use crate::tools::schema::ToolDef;

/// Aider loads nothing by itself: every `.aider.conf.yml` found lists files
/// under `read:`. Append those, resolved relative to the config file.
pub fn finish(_def: &ToolDef, ctx: &Ctx, entries: &mut Vec<Entry>) {
    let confs: Vec<Entry> = entries.iter().filter(|e| e.exists).cloned().collect();
    for conf in confs {
        let Ok(text) = fs::read_to_string(&conf.path) else {
            continue;
        };
        let Ok(yaml) = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&text) else {
            continue;
        };
        let Ok(json) = serde_json::to_value(yaml) else {
            continue;
        };
        let Some(read) = json.get("read") else {
            continue;
        };
        let base = conf.path.parent().unwrap_or(Path::new("/"));
        let opts = EntryOpts::default();
        for raw in file::string_list(read) {
            let path = if raw.starts_with('~') {
                expand_tilde(&raw, &ctx.home)
            } else {
                base.join(&raw)
            };
            entries.push(entry_for(path, conf.scope, &opts, ctx));
        }
    }
}
