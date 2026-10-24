//! Extract `@path` (Claude Code, Gemini CLI, Amp) and `#[[file:path]]` (Kiro) imports.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::model::Import;
use crate::tools::resolve::{expand_tilde, lexical_normalize};

static AT_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:^|[\s(])@((?:~/|\.{1,2}/|/)?[^\s`'"()<>\[\],;:!?]+)"#).unwrap()
});
static CODE_SPAN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`[^`]*`").unwrap());
static KIRO_REF: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"#\[\[file:([^\]:]+)(?::\d+)?\]\]").unwrap());

/// `@path` imports, relative to the importing file. Skips fenced and inline code.
pub fn at_path(content: &str, file: &Path, home: &Path) -> Vec<Import> {
    let base = file.parent().unwrap_or(Path::new("."));
    let mut out = Vec::new();
    let mut in_fence = false;
    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let line = CODE_SPAN.replace_all(line, "");
        for caps in AT_PATH.captures_iter(&line) {
            let raw = caps[1].trim_end_matches(['.', ',', ';', ':']).to_string();
            if raw.is_empty() {
                continue;
            }
            let resolved = if raw.starts_with("~/") {
                expand_tilde(&raw, home)
            } else if raw.starts_with('/') {
                PathBuf::from(&raw)
            } else {
                base.join(&raw)
            };
            let resolved = lexical_normalize(&resolved);
            out.push(Import {
                exists: resolved.is_file(),
                raw: format!("@{raw}"),
                resolved,
                line: i + 1,
            });
        }
    }
    out
}

/// Kiro `#[[file:path]]` references, relative to the workspace root.
pub fn kiro_ref(content: &str, _file: &Path, root: &Path) -> Vec<Import> {
    let mut out = Vec::new();
    for (i, line) in content.lines().enumerate() {
        for caps in KIRO_REF.captures_iter(line) {
            let raw = caps[1].trim().to_string();
            let resolved = lexical_normalize(&root.join(&raw));
            out.push(Import {
                exists: resolved.exists(),
                raw: caps[0].to_string(),
                resolved,
                line: i + 1,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_at_imports_outside_code() {
        let text = "See @docs/style.md and @~/shared.md.\n```\n@not/this.md\n```\nuse `@nor/this.md`\nemail me@example.com\n";
        let imps = at_path(text, Path::new("/repo/CLAUDE.md"), Path::new("/home/u"));
        let raws: Vec<_> = imps.iter().map(|i| i.raw.as_str()).collect();
        assert_eq!(raws, vec!["@docs/style.md", "@~/shared.md"]);
        assert_eq!(imps[0].resolved, PathBuf::from("/repo/docs/style.md"));
        assert_eq!(imps[1].resolved, PathBuf::from("/home/u/shared.md"));
        assert_eq!(imps[0].line, 1);
    }

    #[test]
    fn finds_kiro_refs() {
        let imps = kiro_ref(
            "read #[[file:docs/api.md:12]]",
            Path::new("/w/.kiro/steering/a.md"),
            Path::new("/w"),
        );
        assert_eq!(imps[0].resolved, PathBuf::from("/w/docs/api.md"));
    }
}
