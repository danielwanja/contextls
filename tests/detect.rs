//! Install detection and the "Not detected" collapsing in plain output.

use std::fs;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

/// A fake $HOME where only Codex looks installed, and an empty PATH so no real
/// binaries count. App bundles under /Applications are not isolated, so tests
/// only assert on tools that have no `apps` rule.
fn cmd(home: &TempDir, empty_path: &TempDir) -> Command {
    let mut c = Command::cargo_bin("contextls").unwrap();
    c.env("HOME", home.path())
        .env("PATH", empty_path.path())
        .env_remove("CODEX_HOME")
        .env_remove("CLAUDE_CONFIG_DIR")
        .arg("--no-git");
    c
}

#[test]
fn install_state_from_config_dir() {
    let home = TempDir::new().unwrap();
    let empty = TempDir::new().unwrap();
    fs::create_dir_all(home.path().join(".codex")).unwrap();
    fs::create_dir_all(home.path().join("proj")).unwrap();

    let out = cmd(&home, &empty)
        .args(["--json", "--tool", "codex-cli", "--tool", "kiro"])
        .arg(home.path().join("proj"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    let tools = v["tools"].as_array().unwrap();

    let codex = tools.iter().find(|t| t["id"] == "codex-cli").unwrap();
    assert_eq!(codex["install"]["state"], "installed");
    assert!(
        codex["install"]["evidence"]
            .as_str()
            .unwrap()
            .contains(".codex")
    );

    let kiro = tools.iter().find(|t| t["id"] == "kiro").unwrap();
    assert_eq!(kiro["install"]["state"], "not_installed");
    let looked = kiro["install"]["looked_for"].as_array().unwrap();
    assert!(looked.iter().any(|l| l.as_str().unwrap().contains(".kiro")));
    assert!(
        looked
            .iter()
            .any(|l| l.as_str().unwrap().contains("kiro` on PATH"))
    );
}

#[test]
fn plain_output_collapses_not_detected_tools() {
    let home = TempDir::new().unwrap();
    let empty = TempDir::new().unwrap();
    fs::create_dir_all(home.path().join(".codex")).unwrap();
    fs::create_dir_all(home.path().join("proj")).unwrap();

    let text = |extra: &[&str]| {
        let out = cmd(&home, &empty)
            .args([
                "--plain",
                "--tool",
                "codex-cli",
                "--tool",
                "kiro",
                "--tool",
                "junie",
            ])
            .args(extra)
            .arg(home.path().join("proj"))
            .output()
            .unwrap();
        String::from_utf8(out.stdout).unwrap()
    };

    let t = text(&[]);
    assert!(t.contains("Codex CLI —"));
    assert!(!t.contains("Kiro —"));
    assert!(t.contains("Not detected on this machine (2): Kiro, JetBrains Junie"));

    let t = text(&["--all-tools"]);
    assert!(t.contains("Kiro —"));
    assert!(t.contains("JetBrains Junie —"));
    assert!(!t.contains("Not detected"));
}
