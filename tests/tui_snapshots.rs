//! Render the TUI into a TestBackend and snapshot it.

use std::path::PathBuf;

use contextls::cli::Grouping;
use contextls::model::{Entry, GitStatus, Import, Install, Inventory, LoadMode, Scope, ToolReport};
use contextls::scan::ScanOptions;
use contextls::tools::registry;
use contextls::tui::{App, ui};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

fn entry(tool: &str, scope: Scope, order: usize, path: &str, display: &str, text: &str) -> Entry {
    Entry {
        tool: tool.into(),
        scope,
        order,
        path: PathBuf::from(path),
        display_path: display.into(),
        exists: !text.is_empty(),
        size: text.len() as u64,
        est_tokens: (text.len() as u64).div_ceil(4),
        mtime: None,
        git: if path.starts_with("/repo") {
            GitStatus::Tracked
        } else {
            GitStatus::NotRepo
        },
        load_mode: LoadMode::Always,
        legacy: false,
        frontmatter: None,
        imports: Vec::new(),
        shared_with: Vec::new(),
        warnings: Vec::new(),
        label: None,
    }
}

const REPO_MD: &str = "# Repo rules\n\nUse **tabs**. See @docs/arch.md.\n\n- one\n- two\n";

fn inventory() -> Inventory {
    let mut claude_repo = entry(
        "claude-code",
        Scope::Project,
        2,
        "/repo/CLAUDE.md",
        "./CLAUDE.md",
        REPO_MD,
    );
    claude_repo.imports.push(Import {
        raw: "@docs/arch.md".into(),
        resolved: "/repo/docs/arch.md".into(),
        exists: true,
        line: 3,
    });
    claude_repo.shared_with.push("cursor".into());
    let mut local = entry(
        "claude-code",
        Scope::Local,
        3,
        "/repo/CLAUDE.local.md",
        "./CLAUDE.local.md",
        "my notes\n",
    );
    local.git = GitStatus::Ignored;
    let mut rules = entry(
        "claude-code",
        Scope::Project,
        4,
        "/repo/.claude/rules/db.md",
        "./.claude/rules/db.md",
        "---\npaths: [\"**/*.sql\"]\n---\nnever drop\n",
    );
    rules.load_mode = LoadMode::Conditional {
        globs: vec!["**/*.sql".into()],
    };
    rules.label = Some("rules".into());

    Inventory {
        root: "/repo".into(),
        git_root: Some("/repo".into()),
        home: "/home/u".into(),
        tools: vec![
            ToolReport {
                id: "claude-code".into(),
                name: "Claude Code".into(),
                homepage: "https://example.invalid".into(),
                notes: vec!["note".into()],
                install: Install::Installed {
                    evidence: "found ~/.claude".into(),
                },
                entries: vec![
                    entry(
                        "claude-code",
                        Scope::User,
                        1,
                        "/home/u/.claude/CLAUDE.md",
                        "~/.claude/CLAUDE.md",
                        "",
                    ),
                    claude_repo,
                    local,
                    rules,
                ],
            },
            ToolReport {
                id: "cursor".into(),
                name: "Cursor".into(),
                homepage: "https://example.invalid".into(),
                notes: vec![],
                // No detection rule, but it loads a file here: stays a regular tool.
                install: Install::Unknown,
                entries: vec![entry(
                    "cursor",
                    Scope::Project,
                    1,
                    "/repo/CLAUDE.md",
                    "./CLAUDE.md",
                    REPO_MD,
                )],
            },
            ToolReport {
                id: "kiro".into(),
                name: "Kiro".into(),
                homepage: "https://example.invalid".into(),
                notes: vec![],
                install: Install::NotInstalled {
                    looked_for: vec!["~/.kiro".into(), "`kiro` on PATH".into()],
                },
                entries: vec![entry(
                    "kiro",
                    Scope::Project,
                    1,
                    "/repo/AGENTS.md",
                    "./AGENTS.md",
                    "agents\n",
                )],
            },
        ],
    }
}

fn app(grouping: Grouping) -> App {
    let defs = registry::load().unwrap();
    let mut app = App::new(
        defs,
        "/repo".into(),
        ScanOptions::default(),
        inventory(),
        grouping,
        false,
        None,
    );
    app.preload("/repo/CLAUDE.md".into(), REPO_MD.into());
    app.preload("/repo/CLAUDE.local.md".into(), "my notes\n".into());
    app
}

fn render(app: &mut App, w: u16, h: u16) -> Terminal<TestBackend> {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    app.ensure_content();
    term.draw(|f| ui::draw(f, app)).unwrap();
    term
}

#[test]
fn by_tool_80x24() {
    let mut app = app(Grouping::Tool);
    let term = render(&mut app, 80, 24);
    insta::assert_snapshot!(term.backend());
}

#[test]
fn by_scope_120x40() {
    let mut app = app(Grouping::Scope);
    let term = render(&mut app, 120, 40);
    insta::assert_snapshot!(term.backend());
}

#[test]
fn details_overlay() {
    let mut app = app(Grouping::Tool);
    app.overlay = contextls::tui::app::Overlay::Details;
    let term = render(&mut app, 100, 30);
    insta::assert_snapshot!(term.backend());
}

#[test]
fn missing_entries_hidden_until_toggled() {
    let mut app = app(Grouping::Tool);
    let term = render(&mut app, 80, 24);
    let shown = format!("{}", term.backend());
    assert!(!shown.contains("~/.claude/CLAUDE.md"));

    app.show_missing = true;
    let term = render(&mut app, 80, 24);
    let shown = format!("{}", term.backend());
    assert!(shown.contains("~/.claude/CLAUDE.md"));
}

#[test]
fn not_detected_tools_collapse_until_toggled() {
    let mut app = app(Grouping::Tool);
    let shown = format!("{}", render(&mut app, 80, 24).backend());
    assert!(shown.contains("Not detected"));
    assert!(
        !shown.contains("Kiro"),
        "collapsed group must hide its tools"
    );

    app.toggle_all_tools();
    let shown = format!("{}", render(&mut app, 80, 24).backend());
    assert!(shown.contains("Kiro"));
    assert!(!shown.contains("Not detected"));
}

#[test]
fn filter_narrows_tree() {
    let mut app = app(Grouping::Tool);
    app.filter = "local".into();
    let term = render(&mut app, 80, 24);
    let shown = format!("{}", term.backend());
    assert!(shown.contains("CLAUDE.local.md"));
    assert!(!shown.contains("rules/db.md"));
}

/// Press `s` with a fake provider CLI and check the summary replaces the preview.
#[test]
#[cfg(unix)]
fn summarize_flow_with_fake_provider() {
    use contextls::summarize::{ContentMode, Provider, SummarizerDef};
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("fake-ai");
    std::fs::write(
        &script,
        "#!/bin/sh\nsleep 0.2\necho \"## Summary\"\necho \"- rules about tabs ($(wc -l < /dev/stdin | tr -d ' ') lines)\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let mut app = app(Grouping::Tool);
    app.set_providers(vec![Provider {
        def: SummarizerDef {
            id: "fake".into(),
            name: "Fake AI".into(),
            bin: "fake-ai".into(),
            args: vec!["-p".into(), "{prompt}".into()],
            content: ContentMode::Stdin,
            login: None,
        },
        path: script,
    }]);

    app.summarize(false);
    assert!(app.job_active());
    let shown = format!("{}", render(&mut app, 100, 24).backend());
    assert!(
        shown.contains("summarizing ./CLAUDE.md with Fake AI"),
        "{shown}"
    );

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while app.job_active() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
        app.poll_summary();
    }
    assert!(!app.job_active(), "provider never finished");
    let shown = format!("{}", render(&mut app, 100, 24).backend());
    assert!(
        shown.contains("summary · ./CLAUDE.md · by Fake AI"),
        "{shown}"
    );
    assert!(shown.contains("rules about tabs (6 lines)"), "{shown}");
    assert!(
        app.status
            .as_deref()
            .unwrap()
            .starts_with("summarized by Fake AI")
    );

    // `s` again toggles back to the file, and once more to the cached summary.
    app.summarize(false);
    let shown = format!("{}", render(&mut app, 100, 24).backend());
    assert!(shown.contains("# Repo rules"), "{shown}");
    app.summarize(false);
    let shown = format!("{}", render(&mut app, 100, 24).backend());
    assert!(shown.contains("by Fake AI"), "{shown}");
}
