//! Plain-text output: one table per tool, in load order.

use comfy_table::{Cell, ContentArrangement, Table, presets};

use crate::model::{Entry, Inventory};

use super::{fmt_age, fmt_size, fmt_tokens};

pub fn render(inv: &Inventory, show_missing: bool) -> String {
    let mut out = String::new();
    out.push_str(&format!("{}\n", inv.root.display()));
    if let Some(g) = &inv.git_root
        && g != &inv.root
    {
        out.push_str(&format!("git root: {}\n", g.display()));
    }
    for tool in &inv.tools {
        let present = tool.present().count();
        let missing = tool.entries.len() - present;
        out.push_str(&format!(
            "\n{} — {} file{} · {} tokens",
            tool.name,
            present,
            if present == 1 { "" } else { "s" },
            fmt_tokens(tool.total_tokens()),
        ));
        if missing > 0 && !show_missing {
            out.push_str(&format!(
                " · {missing} expected location{} absent",
                if missing == 1 { "" } else { "s" }
            ));
        }
        out.push('\n');

        let rows: Vec<&Entry> = tool
            .entries
            .iter()
            .filter(|e| show_missing || e.exists)
            .collect();
        if rows.is_empty() {
            continue;
        }
        let mut table = Table::new();
        table
            .load_style(presets::NOTHING)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header([
                "#", "Scope", "Path", "Size", "Tokens", "Modified", "Git", "Notes",
            ]);
        for e in rows {
            table.add_row(vec![
                Cell::new(e.order),
                Cell::new(e.scope.label()),
                Cell::new(&e.display_path),
                Cell::new(if e.exists {
                    fmt_size(e.size)
                } else {
                    "-".into()
                }),
                Cell::new(if e.exists {
                    fmt_tokens(e.est_tokens)
                } else {
                    "-".into()
                }),
                Cell::new(fmt_age(e.mtime)),
                Cell::new(e.git.label()),
                Cell::new(notes(e)),
            ]);
        }
        out.push_str(&table.to_string());
        out.push('\n');
    }
    out
}

pub fn notes(e: &Entry) -> String {
    let mut n = Vec::new();
    if !e.exists {
        n.push("missing".to_string());
    }
    if let Some(b) = e.load_mode.badge() {
        n.push(b);
    }
    if e.legacy {
        n.push("legacy".into());
    }
    if let Some(l) = &e.label {
        n.push(l.clone());
    }
    if !e.imports.is_empty() {
        n.push(format!(
            "{} import{}",
            e.imports.len(),
            if e.imports.len() == 1 { "" } else { "s" }
        ));
    }
    if !e.shared_with.is_empty() {
        n.push(format!("also: {}", e.shared_with.join(", ")));
    }
    if !e.warnings.is_empty() {
        n.push(format!("⚠ {}", e.warnings.len()));
    }
    n.join(" · ")
}
