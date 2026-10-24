use crate::model::{Entry, LoadMode};

/// `.mdc` rules without `alwaysApply` or `globs` are either "Apply Intelligently"
/// (has a description) or "Manual" (@-mention only).
pub fn finish(entries: &mut [Entry]) {
    for e in entries.iter_mut() {
        if e.path.extension().is_none_or(|x| x != "mdc") {
            continue;
        }
        if !matches!(e.load_mode, LoadMode::Always) {
            continue;
        }
        let fm = e.frontmatter.as_ref();
        let always = fm
            .and_then(|f| f.get("alwaysApply"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if always {
            continue;
        }
        let has_description = fm
            .and_then(|f| f.get("description"))
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.trim().is_empty());
        e.load_mode = if has_description {
            LoadMode::Intelligent
        } else {
            LoadMode::Manual
        };
    }
}
