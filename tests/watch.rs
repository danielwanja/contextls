//! The TUI notices files that change under it and keeps the user's place.

use std::fs;
use std::time::{Duration, Instant};

use contextls::scan::{ScanOptions, scan_with};
use contextls::tools::registry;
use contextls::tui::{App, TuiOptions};
use tempfile::TempDir;

fn wait_for_rescan(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while app.rescan_pending() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
        app.poll_rescan();
    }
    assert!(!app.rescan_pending(), "re-scan never finished");
}

fn project_paths(app: &App) -> Vec<String> {
    app.inv
        .tools
        .iter()
        .find(|t| t.id == "claude-code")
        .unwrap()
        .entries
        .iter()
        .filter(|e| e.exists && e.path.starts_with(&app.root))
        .map(|e| e.file_name())
        .collect()
}

#[test]
fn new_file_appears_and_selection_survives() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    fs::write(root.join("CLAUDE.md"), "# rules\n").unwrap();

    let defs = registry::load().unwrap();
    let opts = ScanOptions {
        tools: vec!["claude-code".into()],
        git: false,
    };
    let inv = scan_with(&defs, &root, &opts).unwrap();
    let mut app = App::new(
        defs,
        root.clone(),
        opts,
        inv,
        TuiOptions {
            watch: true,
            ..Default::default()
        },
    );
    assert_eq!(project_paths(&app), vec!["CLAUDE.md"]);
    app.preview_scroll = 3;
    let before = app.selected_entry().map(|(_, e)| e.path.clone());

    // Nothing changed: no re-scan.
    assert!(!app.poll_watch_now());

    // A new file in the watched folder changes the folder's mtime.
    std::thread::sleep(Duration::from_millis(1100)); // coarse-mtime filesystems
    fs::write(root.join("CLAUDE.local.md"), "mine\n").unwrap();
    assert!(app.poll_watch_now(), "folder change not noticed");
    wait_for_rescan(&mut app);

    assert_eq!(project_paths(&app), vec!["CLAUDE.md", "CLAUDE.local.md"]);
    assert_eq!(app.selected_entry().map(|(_, e)| e.path.clone()), before);
    assert_eq!(
        app.preview_scroll, 3,
        "preview scroll must survive a refresh"
    );
    assert!(
        app.status.as_deref().unwrap().starts_with("updated:"),
        "{:?}",
        app.status
    );

    // Editing a known file is noticed through its own mtime.
    std::thread::sleep(Duration::from_millis(1100));
    fs::write(root.join("CLAUDE.md"), "# rules\n- more\n").unwrap();
    assert!(app.poll_watch_now());
    wait_for_rescan(&mut app);
    assert!(app.status.as_deref().unwrap().contains("CLAUDE.md"));
}

/// With a real filesystem watcher, a change is picked up without polling.
#[test]
fn watcher_triggers_rescan_on_its_own() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    fs::write(root.join("CLAUDE.md"), "# rules\n").unwrap();

    let defs = registry::load().unwrap();
    let opts = ScanOptions {
        tools: vec!["claude-code".into()],
        git: false,
    };
    let inv = scan_with(&defs, &root, &opts).unwrap();
    let mut app = App::new(
        defs,
        root.clone(),
        opts,
        inv,
        TuiOptions {
            watch: true,
            ..Default::default()
        },
    );
    if !app.watching() {
        eprintln!("no filesystem watcher available here; polling fallback covers this");
        return;
    }

    // Let the watcher settle, then create a file and only *tick* — no manual poll.
    std::thread::sleep(Duration::from_millis(300));
    fs::write(root.join("CLAUDE.local.md"), "mine\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !app.rescan_pending() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
        app.tick_watch();
    }
    assert!(app.rescan_pending(), "watcher never reported the new file");
    wait_for_rescan(&mut app);
    assert!(project_paths(&app).contains(&"CLAUDE.local.md".to_string()));
    assert!(
        app.status.as_deref().unwrap().starts_with("updated:"),
        "{:?}",
        app.status
    );
}
