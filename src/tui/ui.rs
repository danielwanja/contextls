//! Drawing: tree pane, preview pane, status bar, overlays.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};
use tui_tree_widget::Tree;

use super::app::{App, Focus, Overlay, badges, scope_color};
use crate::model::display_path;
use crate::output::{fmt_age, fmt_size, fmt_tokens};

pub fn draw(frame: &mut Frame, app: &mut App) {
    let [main, status] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(frame.area());
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)]).areas(main);

    draw_tree(frame, app, left);
    draw_preview(frame, app, right);
    draw_status(frame, app, status);
    match app.overlay {
        Overlay::Help => draw_help(frame, main),
        Overlay::Details => draw_details(frame, app, main),
        Overlay::None => {}
    }
}

fn pane_block(title: String, focused: bool) -> Block<'static> {
    let border = if focused {
        Style::new().cyan()
    } else {
        Style::new().dark_gray()
    };
    Block::bordered()
        .border_style(border)
        .title(Line::from(Span::styled(title, Style::new().bold())))
}

fn draw_tree(frame: &mut Frame, app: &mut App, area: Rect) {
    let items = app.items();
    let root = display_path(&app.root, &app.root, &app.inv.home);
    let root = if root == "." {
        app.root.display().to_string()
    } else {
        root
    };
    let mut block = pane_block(format!(" {root} "), app.focus == Focus::Tree);
    if app.filtering || !app.filter.is_empty() {
        let cursor = if app.filtering { "▏" } else { "" };
        block = block.title_bottom(
            Line::from(format!(" /{}{cursor} ", app.filter)).style(Style::new().yellow()),
        );
    }
    let highlight = if app.focus == Focus::Tree {
        Style::new().bg(Color::Blue).fg(Color::White).bold()
    } else {
        Style::new().bg(Color::DarkGray)
    };
    let tree = Tree::new(&items)
        .expect("unique identifiers")
        .block(block)
        .highlight_style(highlight)
        .node_closed_symbol("▸ ")
        .node_open_symbol("▾ ")
        .node_no_children_symbol("  ");
    frame.render_stateful_widget(tree, area, &mut app.tree);
}

fn draw_preview(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Preview;
    let (title, body): (String, Text) = match app.preview_path() {
        Some(path) => {
            let title = display_path(&path, &app.root, &app.inv.home);
            // Imports of the selected entry become links (followed from the details overlay).
            let raws: Vec<String> = if app.adhoc.is_some() {
                Vec::new()
            } else {
                app.selected_entry()
                    .map(|(_, e)| e.imports.iter().map(|i| i.raw.clone()).collect())
                    .unwrap_or_default()
            };
            let body = match app.content(&path) {
                Some(Ok(text)) => highlight_imports(tui_markdown::from_str(text), &raws),
                Some(Err(e)) => Text::from(format!("could not read: {e}")).dark_gray(),
                None => Text::from(""),
            };
            (title, body)
        }
        None => match app.selected_entry() {
            Some((tool, e)) => (
                e.display_path.clone(),
                Text::from(vec![
                    Line::from(""),
                    Line::from(format!(
                        "  Not present. {} would load it from here.",
                        tool.name
                    ))
                    .dark_gray(),
                    Line::from(""),
                    Line::from(format!("  {}", e.path.display())).dark_gray(),
                ]),
            ),
            None => (
                "contextls".into(),
                Text::from(vec![
                    Line::from(""),
                    Line::from("  Select a file on the left to preview it.").dark_gray(),
                    Line::from("  Press ? for keys.").dark_gray(),
                ]),
            ),
        },
    };
    let block = pane_block(format!(" {title} "), focused);
    let para = Paragraph::new(body)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.preview_scroll, 0));
    frame.render_widget(para, area);
}

/// Underline every occurrence of an import token (`@docs/x.md`) in rendered text.
fn highlight_imports<'a>(mut text: Text<'a>, raws: &[String]) -> Text<'a> {
    if raws.is_empty() {
        return text;
    }
    let link = Style::new().cyan().underlined();
    let old_lines = std::mem::take(&mut text.lines);
    text.lines = old_lines
        .into_iter()
        .map(|mut line| {
            let old = std::mem::take(&mut line.spans);
            let mut spans = Vec::with_capacity(old.len());
            for span in old {
                let mut rest: &str = &span.content;
                loop {
                    // Earliest match wins; on a tie the longest token.
                    let hit = raws
                        .iter()
                        .filter_map(|r| rest.find(r.as_str()).map(|i| (i, r.len())))
                        .min_by_key(|(i, len)| (*i, std::cmp::Reverse(*len)));
                    let Some((i, len)) = hit else {
                        if !rest.is_empty() {
                            spans.push(Span::styled(rest.to_string(), span.style));
                        }
                        break;
                    };
                    if i > 0 {
                        spans.push(Span::styled(rest[..i].to_string(), span.style));
                    }
                    spans.push(Span::styled(
                        rest[i..i + len].to_string(),
                        span.style.patch(link),
                    ));
                    rest = &rest[i + len..];
                }
            }
            line.spans = spans;
            line
        })
        .collect();
    text
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let left: Line = if let Some(msg) = &app.status {
        Line::from(Span::styled(format!(" {msg}"), Style::new().yellow()))
    } else if let Some((tool, e)) = app.selected_entry() {
        let mut spans = vec![
            Span::styled(format!(" {} ", tool.name), Style::new().cyan()),
            Span::styled(e.scope.label(), Style::new().fg(scope_color(e.scope))),
        ];
        if e.exists {
            spans.push(Span::raw(format!(
                " · {} · {} · {} · git {}",
                fmt_size(e.size),
                fmt_tokens(e.est_tokens),
                fmt_age(e.mtime),
                e.git.label()
            )));
        } else {
            spans.push(Span::styled(" · not present", Style::new().dark_gray()));
        }
        if let Some(b) = e.load_mode.badge() {
            spans.push(Span::styled(format!(" · {b}"), Style::new().dark_gray()));
        }
        if !e.warnings.is_empty() {
            spans.push(Span::styled(
                format!(" · ⚠ {}", e.warnings[0]),
                Style::new().red(),
            ));
        }
        Line::from(spans)
    } else {
        Line::from("")
    };
    const FULL_HINTS: &str =
        "j/k move · / filter · i details · e edit · g group · m missing · ? help · q quit ";
    const SHORT_HINTS: &str = "? help · q quit ";
    // Keep room for the file info; shrink the hints before clipping it.
    let hints = if left.width() + FULL_HINTS.len() + 2 <= area.width as usize {
        FULL_HINTS
    } else {
        SHORT_HINTS
    };
    let right = Line::from(Span::styled(hints, Style::new().dark_gray())).right_aligned();

    let [l, r] = Layout::horizontal([
        Constraint::Min(10),
        Constraint::Length(right.width() as u16),
    ])
    .areas(area);
    frame.render_widget(Paragraph::new(left), l);
    frame.render_widget(Paragraph::new(right), r);
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width.saturating_sub(2));
    let h = height.min(area.height.saturating_sub(2));
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}

fn draw_help(frame: &mut Frame, area: Rect) {
    let keys = [
        ("↑/k ↓/j", "move (tree) / scroll (preview)"),
        ("←/h →/l", "collapse / expand"),
        ("Enter, Space", "toggle node"),
        ("Home / End", "first / last"),
        ("Tab", "switch pane"),
        ("PgUp/PgDn, u/d", "scroll preview"),
        ("/", "fuzzy filter (Enter keeps, Esc clears)"),
        (
            "i",
            "details: frontmatter, imports (1-9 to follow), warnings",
        ),
        ("e", "open in $VISUAL / $EDITOR"),
        ("g", "group by tool ⇄ by scope"),
        ("m", "show / hide expected-but-missing locations"),
        ("r", "rescan"),
        ("q, Esc", "quit (Esc first clears filter / ad-hoc preview)"),
    ];
    let lines: Vec<Line> = keys
        .iter()
        .map(|(k, d)| {
            Line::from(vec![
                Span::styled(format!("  {k:<16}"), Style::new().yellow()),
                Span::raw(*d),
            ])
        })
        .collect();
    let rect = centered(area, 70, lines.len() as u16 + 2);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::bordered()
                .title(" Keys ")
                .border_style(Style::new().cyan()),
        ),
        rect,
    );
}

fn draw_details(frame: &mut Frame, app: &App, area: Rect) {
    let Some((tool, e)) = app.selected_entry() else {
        return;
    };
    let kv = |k: &str, v: String| {
        Line::from(vec![
            Span::styled(format!("  {k:<12}"), Style::new().dark_gray()),
            Span::raw(v),
        ])
    };
    let mut lines = vec![
        kv("path", e.path.display().to_string()),
        kv("tool", tool.name.clone()),
        kv("scope", e.scope.label().to_string()),
        kv(
            "loads",
            e.load_mode.badge().unwrap_or_else(|| "always".into()),
        ),
    ];
    if e.exists {
        lines.push(kv(
            "size",
            format!("{} · {} tokens", fmt_size(e.size), fmt_tokens(e.est_tokens)),
        ));
        lines.push(kv("modified", fmt_age(e.mtime)));
        lines.push(kv("git", e.git.label().to_string()));
    } else {
        lines.push(kv("status", "not present".into()));
    }
    if !e.shared_with.is_empty() {
        lines.push(kv("also by", e.shared_with.join(", ")));
    }
    let b = badges(e);
    if !b.is_empty() {
        lines.push(kv("badges", b.join(" · ")));
    }
    if let Some(fm) = &e.frontmatter {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Frontmatter",
            Style::new().bold(),
        )));
        let pretty = serde_json::to_string_pretty(fm).unwrap_or_default();
        for l in pretty.lines() {
            lines.push(Line::from(format!("    {l}")));
        }
    }
    if !e.imports.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Imports  (press 1-9 to follow)",
            Style::new().bold(),
        )));
        for (n, imp) in e.imports.iter().enumerate() {
            let mark = if imp.exists { "✓" } else { "✗" };
            let style = if imp.exists {
                Style::new()
            } else {
                Style::new().red()
            };
            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", n + 1), Style::new().yellow()),
                Span::styled(format!("{mark} {}  (line {})", imp.raw, imp.line), style),
            ]));
        }
    }
    if !e.warnings.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Warnings",
            Style::new().bold().red(),
        )));
        for w in &e.warnings {
            lines.push(Line::from(format!("  ⚠ {w}")).red());
        }
    }
    if !tool.notes.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("  About {}", tool.name),
            Style::new().bold(),
        )));
        for n in &tool.notes {
            lines.push(Line::from(format!("  • {n}")).dark_gray());
        }
        lines.push(Line::from(format!("  {}", tool.homepage)).dark_gray());
    }
    let rect = centered(area, 90, lines.len() as u16 + 2);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).block(
            Block::bordered()
                .title(" Details ")
                .border_style(Style::new().cyan()),
        ),
        rect,
    );
}
