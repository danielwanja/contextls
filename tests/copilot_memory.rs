//! GitHub Copilot Chat repository memories in VS Code workspace storage.

#![cfg(target_os = "macos")]

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

fn write(p: &Path, s: &str) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, s).unwrap();
}

#[test]
fn lists_memories_of_the_matching_workspace() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().canonicalize().unwrap();
    let repo = home.join("work/my repo");
    fs::create_dir_all(repo.join("pkg")).unwrap();

    let storage = home.join("Library/Application Support/Code/User/workspaceStorage");
    // Matching workspace: URI is percent-encoded like VS Code writes it.
    let uri = format!("file://{}", repo.to_str().unwrap().replace(' ', "%20"));
    write(
        &storage.join("abc123/workspace.json"),
        &format!(r#"{{"folder": "{uri}"}}"#),
    );
    write(
        &storage.join("abc123/GitHub.copilot-chat/memory-tool/memories/repo/react-apps.md"),
        "# react\n",
    );
    write(
        &storage.join("abc123/GitHub.copilot-chat/memory-tool/memories/repo/gn000.md"),
        "# gn\n",
    );
    // Memories list newest first: make react-apps.md the recent one.
    let repo_mem = storage.join("abc123/GitHub.copilot-chat/memory-tool/memories/repo");
    let now = std::time::SystemTime::now();
    fs::File::open(repo_mem.join("react-apps.md"))
        .unwrap()
        .set_modified(now)
        .unwrap();
    fs::File::open(repo_mem.join("gn000.md"))
        .unwrap()
        .set_modified(now - std::time::Duration::from_secs(86_400))
        .unwrap();
    // Another workspace: must not be listed.
    write(
        &storage.join("other/workspace.json"),
        r#"{"folder": "file:///somewhere/else"}"#,
    );
    write(
        &storage.join("other/GitHub.copilot-chat/memory-tool/memories/repo/nope.md"),
        "nope\n",
    );

    let out = Command::cargo_bin("contextls")
        .unwrap()
        .env("HOME", &home)
        .args(["--json", "--no-git", "--tool", "copilot"])
        .arg(&repo)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    let memories: Vec<&str> = v["tools"][0]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["scope"] == "auto_memory")
        .map(|e| e["path"].as_str().unwrap())
        .collect();
    assert_eq!(memories.len(), 2, "{memories:?}");
    assert!(
        memories[0].ends_with("memories/repo/react-apps.md"),
        "newest first"
    );
    assert!(memories[1].ends_with("memories/repo/gn000.md"));
    assert!(
        v["tools"][0]["entries"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["scope"] == "auto_memory")
            .all(|e| e["label"] == "memory")
    );
}
