//! End-to-end tests: build a fake $HOME with a git repo inside it, run the
//! binary with `--json`, and assert each tool's ordered file list.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as Std;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

struct World {
    _tmp: TempDir,
    home: PathBuf,
    repo: PathBuf,
    sub: PathBuf,
}

fn write(p: &Path, s: &str) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, s).unwrap();
}

fn git(dir: &Path, args: &[&str]) {
    let out = Std::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn slug(p: &Path) -> String {
    p.to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// A home with every tool's user files, and ~/work/repo with pkg/api inside.
fn world() -> World {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().canonicalize().unwrap();
    let repo = home.join("work/repo");
    let sub = repo.join("pkg/api");
    fs::create_dir_all(&sub).unwrap();

    // Claude Code
    write(&home.join(".claude/CLAUDE.md"), "# user\n");
    write(
        &home.join(".claude/rules/style.md"),
        "---\npaths: [\"**/*.ts\"]\n---\nuse tabs\n",
    );
    write(
        &repo.join("CLAUDE.md"),
        "# repo\nRead @docs/arch.md and @missing.md\n",
    );
    write(&repo.join("docs/arch.md"), "arch\n");
    write(&repo.join("CLAUDE.local.md"), "local\n");
    write(&repo.join(".claude/rules/db.md"), "db\n");
    write(&sub.join("CLAUDE.md"), "api\n");
    write(&repo.join("other/CLAUDE.md"), "other\n");
    write(
        &home
            .join(".claude/projects")
            .join(slug(&repo))
            .join("memory/MEMORY.md"),
        "mem\n",
    );

    // Gemini
    write(&home.join(".gemini/GEMINI.md"), "g user\n");
    write(&repo.join("GEMINI.md"), "g repo\n");

    // Codex
    write(&home.join(".codex/AGENTS.md"), "codex user\n");
    write(&repo.join("AGENTS.md"), "agents\n");
    write(&repo.join("pkg/AGENTS.md"), "pkg\n");
    write(&repo.join("pkg/AGENTS.override.md"), "pkg override\n");
    write(&sub.join("AGENTS.md"), "api agents\n");

    // Cursor
    write(
        &repo.join(".cursor/rules/a.mdc"),
        "---\nalwaysApply: true\n---\nA\n",
    );
    write(
        &repo.join(".cursor/rules/b.mdc"),
        "---\nglobs: *.ts,*.tsx\n---\nB\n",
    );
    write(
        &repo.join(".cursor/rules/c.mdc"),
        "---\ndescription: when testing\n---\nC\n",
    );
    write(&repo.join(".cursor/rules/d.mdc"), "D\n");
    write(&repo.join(".cursor/rules/ignored.md"), "not mdc\n");
    write(&repo.join(".cursorrules"), "legacy\n");

    write(&repo.join(".gitignore"), "CLAUDE.local.md\n");
    git(&repo, &["init", "-q"]);
    git(
        &repo,
        &["add", "CLAUDE.md", "AGENTS.md", "GEMINI.md", ".gitignore"],
    );

    World {
        _tmp: tmp,
        home,
        repo,
        sub,
    }
}

fn cmd(w: &World) -> Command {
    let mut c = Command::cargo_bin("contextls").unwrap();
    c.env("HOME", &w.home);
    for var in [
        "CLAUDE_CONFIG_DIR",
        "CODEX_HOME",
        "GEMINI_CLI_HOME",
        "GEMINI_CLI_SYSTEM_SETTINGS_PATH",
        "CLAUDE_CODE_DISABLE_AUTO_MEMORY",
        "CLAUDE_CODE_PROJECT_DIR_NAME",
    ] {
        c.env_remove(var);
    }
    c
}

fn tool_json(w: &World, dir: &Path, tool: &str, extra: &[&str]) -> Value {
    let out = cmd(w)
        .args(["--json", "--all", "--tool", tool])
        .args(extra)
        .arg(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    v["tools"][0].clone()
}

/// Existing entries' paths, `~`-relative, in load order.
fn present(w: &World, tool: &Value) -> Vec<String> {
    tool["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["exists"].as_bool().unwrap())
        .map(|e| {
            let p = e["path"].as_str().unwrap();
            p.strip_prefix(w.home.to_str().unwrap())
                .map(|r| format!("~{r}"))
                .unwrap_or_else(|| p.to_string())
        })
        .collect()
}

fn entry<'a>(tool: &'a Value, suffix: &str) -> &'a Value {
    tool["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["path"].as_str().unwrap().ends_with(suffix))
        .unwrap_or_else(|| panic!("no entry ending in {suffix}"))
}

#[test]
fn claude_load_order_from_subdirectory() {
    let w = world();
    let t = tool_json(&w, &w.sub, "claude-code", &[]);
    let mem = format!("~/.claude/projects/{}/memory/MEMORY.md", slug(&w.repo));
    assert_eq!(
        present(&w, &t),
        vec![
            "~/.claude/CLAUDE.md",
            "~/.claude/rules/style.md",
            "~/work/repo/CLAUDE.md",
            "~/work/repo/.claude/rules/db.md",
            "~/work/repo/CLAUDE.local.md",
            "~/work/repo/pkg/api/CLAUDE.md",
            mem.as_str(),
        ]
    );
    assert_eq!(entry(&t, "CLAUDE.local.md")["scope"], "local");
    assert_eq!(entry(&t, "MEMORY.md")["scope"], "auto_memory");
    assert_eq!(
        entry(&t, "rules/style.md")["load_mode"]["globs"],
        serde_json::json!(["**/*.ts"])
    );

    let imports = entry(&t, "repo/CLAUDE.md")["imports"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(imports.len(), 2);
    assert_eq!(imports[0]["raw"], "@docs/arch.md");
    assert_eq!(imports[0]["exists"], true);
    assert_eq!(imports[1]["exists"], false);
    assert!(
        entry(&t, "repo/CLAUDE.md")["warnings"][0]
            .as_str()
            .unwrap()
            .contains("@missing.md")
    );
}

#[test]
fn claude_descendants_are_on_demand() {
    let w = world();
    let t = tool_json(&w, &w.repo, "claude-code", &[]);
    assert_eq!(
        entry(&t, "pkg/api/CLAUDE.md")["load_mode"]["kind"],
        "on_demand"
    );
    assert_eq!(
        entry(&t, "other/CLAUDE.md")["load_mode"]["kind"],
        "on_demand"
    );
    assert_eq!(entry(&t, "repo/CLAUDE.md")["load_mode"]["kind"], "always");
}

#[test]
fn claude_falls_back_to_agents_md_only_without_claude_md() {
    let w = world();
    let t = tool_json(&w, &w.sub, "claude-code", &[]);
    assert!(!present(&w, &t).iter().any(|p| p.ends_with("AGENTS.md")));

    let tmp = TempDir::new().unwrap();
    let home = tmp.path().canonicalize().unwrap();
    let repo = home.join("r");
    write(&repo.join("AGENTS.md"), "a\n");
    write(&repo.join("x/AGENTS.md"), "x\n");
    let w2 = World {
        _tmp: tmp,
        home,
        repo: repo.clone(),
        sub: repo.join("x"),
    };
    let t = tool_json(&w2, &w2.sub, "claude-code", &[]);
    assert_eq!(present(&w2, &t), vec!["~/r/AGENTS.md", "~/r/x/AGENTS.md"]);
    assert_eq!(entry(&t, "r/AGENTS.md")["load_mode"]["kind"], "fallback");
}

#[test]
fn claude_config_dir_override() {
    let w = world();
    write(&w.home.join("alt/CLAUDE.md"), "alt\n");
    let out = cmd(&w)
        .env("CLAUDE_CONFIG_DIR", w.home.join("alt"))
        .env("CLAUDE_CODE_DISABLE_AUTO_MEMORY", "1")
        .args(["--json", "--tool", "claude-code"])
        .arg(&w.sub)
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    let paths = present(&w, &v["tools"][0]);
    assert_eq!(paths[0], "~/alt/CLAUDE.md");
    assert!(!paths.iter().any(|p| p.contains("/memory/")));
}

#[test]
fn codex_one_file_per_directory_root_first() {
    let w = world();
    let t = tool_json(&w, &w.sub, "codex-cli", &[]);
    assert_eq!(
        present(&w, &t),
        vec![
            "~/.codex/AGENTS.md",
            "~/work/repo/AGENTS.md",
            "~/work/repo/pkg/AGENTS.override.md",
            "~/work/repo/pkg/api/AGENTS.md",
        ]
    );
}

#[test]
fn codex_honours_config_fallback_filenames() {
    let w = world();
    write(
        &w.home.join(".codex/config.toml"),
        "project_doc_fallback_filenames = [\"TEAM.md\"]\n",
    );
    write(&w.repo.join("pkg/api/x/TEAM.md"), "team\n");
    let t = tool_json(&w, &w.repo.join("pkg/api/x"), "codex-cli", &[]);
    assert!(present(&w, &t).last().unwrap().ends_with("x/TEAM.md"));
}

#[test]
fn gemini_reads_context_filename_setting() {
    let w = world();
    let t = tool_json(&w, &w.repo, "gemini-cli", &[]);
    assert_eq!(
        present(&w, &t),
        vec!["~/.gemini/GEMINI.md", "~/work/repo/GEMINI.md"]
    );

    write(
        &w.repo.join(".gemini/settings.json"),
        r#"{"context": {"fileName": ["AGENTS.md", "GEMINI.md"]}}"#,
    );
    let t = tool_json(&w, &w.repo, "gemini-cli", &[]);
    let paths = present(&w, &t);
    assert!(paths.contains(&"~/work/repo/AGENTS.md".to_string()));
    assert!(paths.contains(&"~/work/repo/GEMINI.md".to_string()));
}

#[test]
fn cursor_rule_activation_modes() {
    let w = world();
    let t = tool_json(&w, &w.repo, "cursor", &[]);
    assert_eq!(entry(&t, "a.mdc")["load_mode"]["kind"], "always");
    assert_eq!(
        entry(&t, "b.mdc")["load_mode"]["globs"],
        serde_json::json!(["*.ts", "*.tsx"])
    );
    assert_eq!(entry(&t, "c.mdc")["load_mode"]["kind"], "intelligent");
    assert_eq!(entry(&t, "d.mdc")["load_mode"]["kind"], "manual");
    assert_eq!(entry(&t, ".cursorrules")["legacy"], true);
    assert!(!present(&w, &t).iter().any(|p| p.ends_with("ignored.md")));
    // Same file as Codex's project doc (and every other AGENTS.md reader).
    let shared = entry(&t, "repo/AGENTS.md")["shared_with"]
        .as_array()
        .unwrap();
    assert!(shared.contains(&serde_json::json!("codex-cli")));
    assert!(!shared.contains(&serde_json::json!("cursor")));
}

#[test]
fn git_status_per_file() {
    let w = world();
    let t = tool_json(&w, &w.repo, "claude-code", &[]);
    assert_eq!(entry(&t, "repo/CLAUDE.md")["git"], "tracked");
    assert_eq!(entry(&t, "CLAUDE.local.md")["git"], "ignored");
    assert_eq!(entry(&t, "rules/db.md")["git"], "untracked");
    assert_eq!(
        entry(&t, "~/.claude/CLAUDE.md".trim_start_matches('~'))["git"],
        "not_repo"
    );

    let t = tool_json(&w, &w.repo, "claude-code", &["--no-git"]);
    assert_eq!(entry(&t, "repo/CLAUDE.md")["git"], "unknown");
}

#[test]
fn plain_output_lists_tools_and_hides_missing_by_default() {
    let w = world();
    let out = cmd(&w).arg("--plain").arg(&w.repo).output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("Claude Code —"));
    assert!(text.contains("Codex CLI —"));
    assert!(text.contains("./CLAUDE.md"));
    assert!(!text.contains("missing"));
    assert!(text.contains("expected location"));
}

#[test]
fn list_tools() {
    let w = world();
    cmd(&w)
        .arg("--list-tools")
        .assert()
        .success()
        .stdout(predicates::str::contains("claude-code"));
}
