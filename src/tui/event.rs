//! Event loop and key handling.

use std::time::Duration;

use anyhow::Result;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

use super::app::{App, Focus, Overlay};
use super::{editor, ui};

const PAGE: u16 = 10;

pub fn run_loop(terminal: &mut DefaultTerminal, app: &mut App) -> Result<()> {
    loop {
        app.tick_watch();
        app.poll_rescan();
        app.poll_summary();
        app.ensure_content();
        terminal.draw(|f| ui::draw(f, app))?;
        if app.quit {
            return Ok(());
        }
        // Redraw often while a summary or re-scan is running; otherwise wake
        // once a second so the watcher gets its turn (ratatui only repaints diffs).
        let wait = if app.job_active() || app.rescan_pending() {
            Duration::from_millis(120)
        } else if app.watch {
            Duration::from_secs(1)
        } else {
            Duration::from_secs(60)
        };
        if !event::poll(wait)? {
            continue;
        }
        if let Event::Key(key) = event::read()?
            && key.is_press()
        {
            app.status = None;
            handle_key(terminal, app, key)?;
        }
    }
}

fn handle_key(terminal: &mut DefaultTerminal, app: &mut App, key: KeyEvent) -> Result<()> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.quit = true;
        return Ok(());
    }
    if app.filtering {
        filter_key(app, key);
        return Ok(());
    }
    if app.overlay != Overlay::None {
        overlay_key(app, key);
        return Ok(());
    }

    match key.code {
        KeyCode::Char('q') => app.quit = true,
        KeyCode::Esc => {
            if app.adhoc.is_some() {
                app.adhoc = None;
            } else if !app.filter.is_empty() {
                app.filter.clear();
            } else {
                app.quit = true;
            }
        }
        KeyCode::Char('?') => app.overlay = Overlay::Help,
        KeyCode::Char('i') => {
            if app.selected_entry().is_some() {
                app.overlay = Overlay::Details;
            }
        }
        KeyCode::Char('/') => app.filtering = true,
        KeyCode::Tab => {
            app.focus = match app.focus {
                Focus::Tree => Focus::Preview,
                Focus::Preview => Focus::Tree,
            }
        }
        KeyCode::Char('g') => app.toggle_grouping(),
        KeyCode::Char('a') => app.toggle_all_tools(),
        KeyCode::Char('m') => {
            app.show_missing = !app.show_missing;
            app.status = Some(
                if app.show_missing {
                    "showing expected-but-missing locations"
                } else {
                    "hiding missing locations"
                }
                .into(),
            );
        }
        KeyCode::Char('r') => app.rescan(),
        KeyCode::Char('o') => {
            let target = app
                .preview_path()
                .or_else(|| app.selected_entry().map(|(_, e)| e.path.clone()));
            app.status = Some(match target {
                Some(p) => match editor::reveal(&p) {
                    Ok(()) => format!("revealed {}", p.display()),
                    Err(e) => format!("could not reveal: {e}"),
                },
                None => "select a file first".into(),
            });
        }
        KeyCode::Char('s') => app.summarize(false),
        KeyCode::Char('S') => app.summarize(true),
        KeyCode::Char('p') => {
            if app.providers.is_empty() {
                app.status = Some("no summarizer CLI found on PATH".into());
            } else {
                app.pick_cursor = app.provider;
                app.overlay = Overlay::Pick;
            }
        }
        KeyCode::Char('e') => {
            if let Some(path) = app.preview_path() {
                editor::open(terminal, &path)?;
                app.rescan();
            }
        }
        KeyCode::PageDown | KeyCode::Char('d') => scroll(app, PAGE as i32),
        KeyCode::PageUp | KeyCode::Char('u') => scroll(app, -(PAGE as i32)),
        KeyCode::Down | KeyCode::Char('j') => match app.focus {
            Focus::Tree => {
                app.tree.key_down();
                on_move(app);
            }
            Focus::Preview => scroll(app, 1),
        },
        KeyCode::Up | KeyCode::Char('k') => match app.focus {
            Focus::Tree => {
                app.tree.key_up();
                on_move(app);
            }
            Focus::Preview => scroll(app, -1),
        },
        KeyCode::Left | KeyCode::Char('h') => {
            app.tree.key_left();
            on_move(app);
        }
        KeyCode::Right | KeyCode::Char('l') => {
            app.tree.key_right();
            on_move(app);
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            app.tree.toggle_selected();
            on_move(app);
        }
        KeyCode::Home => {
            app.tree.select_first();
            on_move(app);
        }
        KeyCode::End => {
            app.tree.select_last();
            on_move(app);
        }
        _ => {}
    }
    Ok(())
}

fn on_move(app: &mut App) {
    app.preview_scroll = 0;
    app.adhoc = None;
    app.show_summary = false;
}

fn scroll(app: &mut App, delta: i32) {
    app.preview_scroll = (app.preview_scroll as i32 + delta).max(0) as u16;
}

fn filter_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.filtering = false;
            app.filter.clear();
        }
        KeyCode::Enter => app.filtering = false,
        KeyCode::Backspace => {
            app.filter.pop();
        }
        KeyCode::Down => {
            app.tree.key_down();
        }
        KeyCode::Up => {
            app.tree.key_up();
        }
        KeyCode::Char(c) => app.filter.push(c),
        _ => {}
    }
}

fn overlay_key(app: &mut App, key: KeyEvent) {
    if app.overlay == Overlay::Pick {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('p') => app.overlay = Overlay::None,
            KeyCode::Down | KeyCode::Char('j') => {
                app.pick_cursor = (app.pick_cursor + 1).min(app.providers.len().saturating_sub(1));
            }
            KeyCode::Up | KeyCode::Char('k') => {
                app.pick_cursor = app.pick_cursor.saturating_sub(1);
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                app.choose_provider(app.pick_cursor);
                app.overlay = Overlay::None;
            }
            KeyCode::Char(c @ '1'..='9') => {
                app.choose_provider(c.to_digit(10).unwrap() as usize - 1);
                app.overlay = Overlay::None;
            }
            _ => {}
        }
        return;
    }
    match key.code {
        KeyCode::Esc
        | KeyCode::Char('q')
        | KeyCode::Char('?')
        | KeyCode::Char('i')
        | KeyCode::Enter => {
            app.overlay = Overlay::None;
        }
        KeyCode::Char(c @ '1'..='9') if app.overlay == Overlay::Details => {
            let n = c.to_digit(10).unwrap() as usize - 1;
            let target = app
                .selected_entry()
                .and_then(|(_, e)| e.imports.get(n))
                .map(|imp| imp.resolved.clone());
            if let Some(path) = target {
                app.overlay = Overlay::None;
                app.jump_to_path(&path);
            }
        }
        _ => {}
    }
}
