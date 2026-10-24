//! GitHub Copilot Chat keeps per-repository "memory tool" notes in VS Code's
//! workspace storage, keyed by a hash directory whose `workspace.json` names
//! the workspace folder as a `file://` URI.

use std::fs;
use std::path::{Path, PathBuf};

use crate::model::{Entry, Scope};
use crate::scan::Ctx;
use crate::tools::resolve::{EntryOpts, entry_for};
use crate::tools::schema::ToolDef;

const MEMORY_SUBDIR: &str = "GitHub.copilot-chat/memory-tool/memories";

pub fn finish(_def: &ToolDef, ctx: &Ctx, entries: &mut Vec<Entry>) {
    // The workspace is usually the repo root even when inspecting a subfolder.
    let mut wanted: Vec<PathBuf> = vec![ctx.root.clone()];
    if let Some(g) = &ctx.git_root
        && g != &ctx.root
    {
        wanted.push(g.clone());
    }
    let opts = EntryOpts {
        label: Some("memory".into()),
        ..Default::default()
    };
    for storage in storage_roots(ctx) {
        let Ok(rd) = fs::read_dir(&storage) else {
            continue;
        };
        let mut hashes: Vec<PathBuf> = rd.flatten().map(|d| d.path()).collect();
        hashes.sort();
        for hash in hashes {
            let Some(folder) = workspace_folder(&hash) else {
                continue;
            };
            if !wanted.iter().any(|w| same_path(w, &folder)) {
                continue;
            }
            let memories = hash.join(MEMORY_SUBDIR);
            let Ok(kinds) = fs::read_dir(&memories) else {
                continue;
            };
            let mut dirs: Vec<PathBuf> = kinds
                .flatten()
                .map(|d| d.path())
                .filter(|p| p.is_dir())
                .collect();
            dirs.sort();
            for dir in dirs {
                let Ok(files) = fs::read_dir(&dir) else {
                    continue;
                };
                let mut files: Vec<PathBuf> = files
                    .flatten()
                    .map(|d| d.path())
                    .filter(|p| {
                        p.is_file()
                            && p.extension().is_some_and(|e| e == "md")
                            && !p
                                .file_name()
                                .is_some_and(|n| n.to_string_lossy().starts_with('.'))
                    })
                    .collect();
                files.sort();
                for f in files {
                    entries.push(entry_for(f, Scope::AutoMemory, &opts, ctx));
                }
            }
        }
    }
}

/// VS Code's `workspaceStorage` directories (stable and Insiders) on this platform.
fn storage_roots(ctx: &Ctx) -> Vec<PathBuf> {
    let user_dirs: Vec<PathBuf> = match ctx.platform {
        "macos" => vec![ctx.home.join("Library/Application Support")],
        "windows" => ctx
            .env
            .get("APPDATA")
            .map(PathBuf::from)
            .into_iter()
            .collect(),
        _ => vec![ctx.home.join(".config")],
    };
    user_dirs
        .iter()
        .flat_map(|base| {
            ["Code", "Code - Insiders", "VSCodium"]
                .into_iter()
                .map(move |app| base.join(app).join("User/workspaceStorage"))
        })
        .filter(|p| p.is_dir())
        .collect()
}

/// The folder a workspace-storage hash belongs to, from its `workspace.json`.
fn workspace_folder(hash_dir: &Path) -> Option<PathBuf> {
    let text = fs::read_to_string(hash_dir.join("workspace.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let uri = v.get("folder")?.as_str()?;
    let path = uri.strip_prefix("file://")?;
    // `file:///C:/x` on Windows; plain `/x` elsewhere.
    let path = path
        .strip_prefix('/')
        .filter(|p| p.get(1..2) == Some(":"))
        .unwrap_or(path);
    Some(PathBuf::from(percent_decode(path)))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(h) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(h);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn same_path(a: &Path, b: &Path) -> bool {
    let canon = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    canon(a) == canon(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_percent_escapes() {
        assert_eq!(
            percent_decode("/Users/d/My%20Repo%2Bx"),
            "/Users/d/My Repo+x"
        );
        assert_eq!(percent_decode("plain"), "plain");
        assert_eq!(percent_decode("trailing%2"), "trailing%2");
    }
}
