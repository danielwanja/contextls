//! End-to-end tests for the tools added after the first four. Each test builds
//! its own fake $HOME (with ~/work/repo as a git repo) and asserts the ordered
//! file list the binary reports.

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
}

impl World {
    fn new() -> World {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().canonicalize().unwrap();
        let repo = home.join("work/repo");
        fs::create_dir_all(&repo).unwrap();
        let out = Std::new("git")
            .args(["-C", repo.to_str().unwrap(), "init", "-q"])
            .output()
            .unwrap();
        assert!(out.status.success());
        World {
            _tmp: tmp,
            home,
            repo,
        }
    }

    fn home(&self, rel: &str, text: &str) -> &Self {
        write(&self.home.join(rel), text);
        self
    }

    fn repo(&self, rel: &str, text: &str) -> &Self {
        write(&self.repo.join(rel), text);
        self
    }

    /// The tool's JSON report for `dir` (default: the repo), missing entries included.
    fn tool(&self, tool: &str, dir: Option<&Path>) -> Value {
        let mut c = Command::cargo_bin("contextls").unwrap();
        c.env("HOME", &self.home);
        for var in [
            "CLAUDE_CONFIG_DIR",
            "CODEX_HOME",
            "GEMINI_CLI_HOME",
            "CLAUDE_CODE_DISABLE_AUTO_MEMORY",
            "AMP_IGNORE_GUIDANCE_FILES",
        ] {
            c.env_remove(var);
        }
        let out = c
            .args(["--json", "--all", "--tool", tool])
            .arg(dir.unwrap_or(&self.repo))
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
    fn present(&self, tool: &Value) -> Vec<String> {
        tool["entries"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["exists"].as_bool().unwrap())
            .map(|e| {
                let p = e["path"].as_str().unwrap();
                p.strip_prefix(self.home.to_str().unwrap())
                    .map(|r| format!("~{r}"))
                    .unwrap_or_else(|| p.to_string())
            })
            .collect()
    }
}

fn write(p: &Path, s: &str) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, s).unwrap();
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
fn all_fourteen_tools_are_listed() {
    let out = Command::cargo_bin("contextls")
        .unwrap()
        .arg("--list-tools")
        .output()
        .unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    for id in [
        "claude-code",
        "gemini-cli",
        "codex-cli",
        "cursor",
        "copilot",
        "windsurf",
        "cline",
        "roo-code",
        "aider",
        "zed",
        "kiro",
        "junie",
        "amp",
        "opencode",
    ] {
        assert!(text.contains(id), "missing {id}");
    }
    assert_eq!(text.lines().count(), 14);
}

#[test]
fn copilot() {
    let w = World::new();
    w.home(".copilot/copilot-instructions.md", "me\n")
        .home(
            ".copilot/instructions/a.instructions.md",
            "---\napplyTo: \"**/*.py\"\n---\npy\n",
        )
        .repo(".github/copilot-instructions.md", "repo\n")
        .repo(
            ".github/instructions/ts.instructions.md",
            "---\napplyTo: \"**/*.ts, **/*.tsx\"\n---\nts\n",
        )
        .repo("AGENTS.md", "agents\n")
        .repo("pkg/AGENTS.md", "nested\n")
        .repo("CLAUDE.md", "claude\n")
        .repo(".github/prompts/review.prompt.md", "prompt\n");
    let t = w.tool("copilot", None);
    assert_eq!(
        w.present(&t),
        vec![
            "~/.copilot/copilot-instructions.md",
            "~/.copilot/instructions/a.instructions.md",
            "~/work/repo/.github/copilot-instructions.md",
            "~/work/repo/.github/instructions/ts.instructions.md",
            "~/work/repo/AGENTS.md",
            "~/work/repo/pkg/AGENTS.md",
            "~/work/repo/CLAUDE.md",
            "~/work/repo/.github/prompts/review.prompt.md",
        ]
    );
    assert_eq!(
        entry(&t, "ts.instructions.md")["load_mode"]["globs"],
        serde_json::json!(["**/*.ts", "**/*.tsx"])
    );
    assert_eq!(entry(&t, "pkg/AGENTS.md")["load_mode"]["kind"], "on_demand");
    assert_eq!(entry(&t, "repo/CLAUDE.md")["load_mode"]["kind"], "fallback");
    assert_eq!(entry(&t, "review.prompt.md")["load_mode"]["kind"], "manual");
}

#[test]
fn windsurf() {
    let w = World::new();
    w.home(".codeium/windsurf/memories/global_rules.md", "global\n")
        .home(".codeium/windsurf/memories/auto-1.md", "remembered\n")
        .repo(
            ".windsurf/rules/ts.md",
            "---\ntrigger: glob\nglobs: \"*.ts\"\n---\nts\n",
        )
        .repo(
            ".windsurf/rules/always.md",
            "---\ntrigger: always_on\n---\nalways\n",
        )
        .repo(
            ".windsurf/rules/ask.md",
            "---\ntrigger: model_decision\ndescription: db work\n---\ndb\n",
        )
        .repo(".windsurfrules", "legacy\n")
        .repo("AGENTS.md", "agents\n");
    let t = w.tool("windsurf", None);
    assert_eq!(
        w.present(&t),
        vec![
            "~/.codeium/windsurf/memories/global_rules.md",
            "~/.codeium/windsurf/memories/auto-1.md",
            "~/work/repo/.windsurf/rules/always.md",
            "~/work/repo/.windsurf/rules/ask.md",
            "~/work/repo/.windsurf/rules/ts.md",
            "~/work/repo/.windsurfrules",
            "~/work/repo/AGENTS.md",
        ]
    );
    assert_eq!(entry(&t, "global_rules.md")["scope"], "user");
    assert_eq!(entry(&t, "auto-1.md")["scope"], "auto_memory");
    assert_eq!(
        entry(&t, "rules/ts.md")["load_mode"]["globs"],
        serde_json::json!(["*.ts"])
    );
    assert_eq!(entry(&t, "rules/always.md")["load_mode"]["kind"], "always");
    assert_eq!(
        entry(&t, "rules/ask.md")["load_mode"]["kind"],
        "intelligent"
    );
    assert_eq!(entry(&t, ".windsurfrules")["legacy"], true);
}

#[test]
fn cline() {
    let w = World::new();
    w.home("Documents/Cline/Rules/global.md", "g\n")
        .home(".agents/AGENTS.md", "agents home\n")
        .repo(
            ".clinerules/style.md",
            "---\npaths: [\"src/**\"]\n---\nstyle\n",
        )
        .repo(".cline/rules/more.md", "more\n")
        .repo("AGENTS.md", "agents\n")
        .repo(".cursorrules", "cursor legacy\n");
    let t = w.tool("cline", None);
    assert_eq!(
        w.present(&t),
        vec![
            "~/Documents/Cline/Rules/global.md",
            "~/.agents/AGENTS.md",
            "~/work/repo/.clinerules/style.md",
            "~/work/repo/.cline/rules/more.md",
            "~/work/repo/AGENTS.md",
            "~/work/repo/.cursorrules",
        ]
    );
    assert_eq!(
        entry(&t, "style.md")["load_mode"]["globs"],
        serde_json::json!(["src/**"])
    );
    assert_eq!(entry(&t, ".cursorrules")["legacy"], true);
}

#[test]
fn roo_code() {
    let w = World::new();
    w.home(".roo/rules/g.md", "g\n")
        .home(".roo/rules-code/gc.md", "gc\n")
        .repo(".roo/rules/general.md", "general\n")
        .repo(".roo/rules-code/code.md", "code\n")
        .repo("AGENTS.md", "agents\n")
        .repo(".roorules", "legacy\n");
    let t = w.tool("roo-code", None);
    assert_eq!(
        w.present(&t),
        vec![
            "~/.roo/rules-code/gc.md",
            "~/.roo/rules/g.md",
            "~/work/repo/.roo/rules-code/code.md",
            "~/work/repo/AGENTS.md",
            "~/work/repo/.roo/rules/general.md",
            "~/work/repo/.roorules",
        ]
    );
    assert_eq!(entry(&t, ".roorules")["load_mode"]["kind"], "fallback");
    assert_eq!(entry(&t, ".roorules")["legacy"], true);
}

#[test]
fn aider_reads_come_from_config() {
    let w = World::new();
    w.home(".aider.conf.yml", "read: ~/CONVENTIONS.md\n")
        .home("CONVENTIONS.md", "home conventions\n")
        .repo(
            ".aider.conf.yml",
            "read:\n  - CONVENTIONS.md\n  - docs/style.md\n",
        )
        .repo("CONVENTIONS.md", "repo conventions\n")
        .repo("docs/style.md", "style\n");
    let t = w.tool("aider", None);
    assert_eq!(
        w.present(&t),
        vec![
            "~/.aider.conf.yml",
            "~/work/repo/.aider.conf.yml",
            "~/CONVENTIONS.md",
            "~/work/repo/CONVENTIONS.md",
            "~/work/repo/docs/style.md",
        ]
    );
    assert_eq!(
        entry(&t, "~/CONVENTIONS.md".trim_start_matches('~'))["scope"],
        "user"
    );
    assert_eq!(entry(&t, "repo/CONVENTIONS.md")["scope"], "project");
    assert_eq!(entry(&t, "work/repo/.aider.conf.yml")["label"], "config");
}

#[test]
fn zed_first_match_only() {
    let w = World::new();
    w.home(".config/zed/AGENTS.md", "personal\n")
        .repo(".rules", "rules\n")
        .repo("AGENTS.md", "agents\n")
        .repo("CLAUDE.md", "claude\n");
    let t = w.tool("zed", None);
    assert_eq!(
        w.present(&t),
        vec!["~/.config/zed/AGENTS.md", "~/work/repo/.rules"]
    );

    fs::remove_file(w.repo.join(".rules")).unwrap();
    let t = w.tool("zed", None);
    assert_eq!(
        w.present(&t),
        vec!["~/.config/zed/AGENTS.md", "~/work/repo/AGENTS.md"]
    );
}

#[test]
fn kiro_steering() {
    let w = World::new();
    w.home(".kiro/steering/global.md", "g\n")
        .repo(".kiro/steering/product.md", "---\ninclusion: always\n---\nproduct\n")
        .repo(
            ".kiro/steering/api.md",
            "---\ninclusion: fileMatch\nfileMatchPattern: \"src/api/**\"\n---\nsee #[[file:docs/api.md]]\n",
        )
        .repo(".kiro/steering/manual.md", "---\ninclusion: manual\n---\nm\n")
        .repo("docs/api.md", "api\n")
        .repo("AGENTS.md", "agents\n");
    let t = w.tool("kiro", None);
    assert_eq!(
        w.present(&t),
        vec![
            "~/.kiro/steering/global.md",
            "~/work/repo/.kiro/steering/api.md",
            "~/work/repo/.kiro/steering/manual.md",
            "~/work/repo/.kiro/steering/product.md",
            "~/work/repo/AGENTS.md",
        ]
    );
    assert_eq!(entry(&t, "product.md")["load_mode"]["kind"], "always");
    assert_eq!(
        entry(&t, "api.md")["load_mode"]["globs"],
        serde_json::json!(["src/api/**"])
    );
    assert_eq!(entry(&t, "manual.md")["load_mode"]["kind"], "manual");
    let imports = entry(&t, "steering/api.md")["imports"].as_array().unwrap();
    assert_eq!(imports[0]["raw"], "#[[file:docs/api.md]]");
    assert_eq!(imports[0]["exists"], true);
}

#[test]
fn junie() {
    let w = World::new();
    w.home(".junie/AGENTS.md", "personal\n")
        .repo(".junie/AGENTS.md", "junie agents\n")
        .repo("AGENTS.md", "root agents\n")
        .repo(".junie/playbook.md", "playbook\n")
        .repo(".junie/rules/r.md", "r\n")
        .repo(".junie/guidelines.md", "legacy\n");
    let t = w.tool("junie", None);
    assert_eq!(
        w.present(&t),
        vec![
            "~/.junie/AGENTS.md",
            "~/work/repo/.junie/AGENTS.md",
            "~/work/repo/.junie/playbook.md",
            "~/work/repo/.junie/rules/r.md",
            "~/work/repo/.junie/guidelines.md",
        ]
    );
    assert_eq!(entry(&t, "guidelines.md")["legacy"], true);
}

#[test]
fn amp_walks_to_home() {
    let w = World::new();
    w.home(".config/amp/AGENTS.md", "amp user\n")
        .home("work/AGENTS.md", "work\n")
        .repo("CLAUDE.md", "claude fallback\n")
        .repo("pkg/AGENT.md", "pkg\n");
    let t = w.tool("amp", None);
    assert_eq!(
        w.present(&t),
        vec![
            "~/.config/amp/AGENTS.md",
            "~/work/AGENTS.md",
            "~/work/repo/CLAUDE.md",
            "~/work/repo/pkg/AGENT.md",
        ]
    );
    assert_eq!(entry(&t, "pkg/AGENT.md")["load_mode"]["kind"], "on_demand");
}

#[test]
fn opencode() {
    let w = World::new();
    w.home(".config/opencode/AGENTS.md", "oc\n")
        .home(".claude/CLAUDE.md", "claude user\n")
        .repo("CLAUDE.md", "claude\n")
        .repo("pkg/AGENTS.md", "pkg\n");
    let t = w.tool("opencode", Some(&w.repo.join("pkg")));
    assert_eq!(
        w.present(&t),
        vec![
            "~/.config/opencode/AGENTS.md",
            "~/.claude/CLAUDE.md",
            "~/work/repo/CLAUDE.md",
            "~/work/repo/pkg/AGENTS.md",
        ]
    );
    assert_eq!(
        entry(&t, "~/.claude/CLAUDE.md".trim_start_matches('~'))["load_mode"]["kind"],
        "fallback"
    );
}
