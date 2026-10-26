//! Render the TUI into a styled HTML "terminal window" for documentation
//! screenshots (then rasterise with a headless browser).
//!
//! cargo run --example screenshot -- <folder> <view> <out.html> [cols] [rows]
//! views: tree | scope | details | help | all-tools | pick

use std::fmt::Write as _;
use std::path::PathBuf;

use contextls::cli::Grouping;
use contextls::scan::{ScanOptions, scan_with};
use contextls::tools::registry;
use contextls::tui::app::Overlay;
use contextls::tui::{App, TuiOptions, ui};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::{Color, Modifier};

fn css_color(c: Color, fg: bool) -> Option<&'static str> {
    Some(match c {
        Color::Reset => return None,
        Color::Black => "#1e1f22",
        Color::Red => "#e06c75",
        Color::Green => "#98c379",
        Color::Yellow => "#e5c07b",
        Color::Blue => {
            if fg {
                "#61afef"
            } else {
                "#3b6ea8"
            }
        }
        Color::Magenta => "#c678dd",
        Color::Cyan => "#56b6c2",
        Color::Gray => "#abb2bf",
        Color::DarkGray => {
            if fg {
                "#7f848e"
            } else {
                "#3a3f4b"
            }
        }
        Color::LightRed => "#ef7a85",
        Color::LightGreen => "#a9d48a",
        Color::LightYellow => "#f0cf8c",
        Color::LightBlue => "#7cbdf5",
        Color::LightMagenta => "#d58ee9",
        Color::LightCyan => "#6cc7d2",
        Color::White => "#f2f2f2",
        _ => "#d4d4d4",
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: screenshot <folder> <view> <out.html> [cols] [rows]");
        std::process::exit(2);
    }
    let folder = PathBuf::from(&args[1]);
    let view = args[2].as_str();
    let out = PathBuf::from(&args[3]);
    let cols: u16 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(128);
    let rows: u16 = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(34);

    let defs = registry::load().expect("definitions");
    let opts = ScanOptions {
        tools: vec![],
        git: true,
    };
    let inv = scan_with(&defs, &folder, &opts).expect("scan");
    let grouping = if view == "scope" {
        Grouping::Scope
    } else {
        Grouping::Tool
    };
    let mut app = App::new(
        defs,
        folder,
        opts,
        inv,
        TuiOptions {
            grouping,
            ..Default::default()
        },
    );
    match view {
        "details" => app.overlay = Overlay::Details,
        "help" => app.overlay = Overlay::Help,
        "pick" => app.overlay = Overlay::Pick,
        "all-tools" => app.toggle_all_tools(),
        _ => {}
    }
    app.status = None;
    app.ensure_content();

    let mut term = Terminal::new(TestBackend::new(cols, rows)).unwrap();
    term.draw(|f| ui::draw(f, &mut app)).unwrap();
    let buf = term.backend().buffer();

    let mut html = String::new();
    for y in 0..rows {
        let mut x = 0;
        while x < cols {
            let cell = &buf[(x, y)];
            let style = cell.style();
            let mut run = String::new();
            let mut x2 = x;
            while x2 < cols && buf[(x2, y)].style() == style {
                run.push_str(buf[(x2, y)].symbol());
                x2 += 1;
            }
            let mut css = String::new();
            if let Some(c) = css_color(style.fg.unwrap_or(Color::Reset), true) {
                let _ = write!(css, "color:{c};");
            }
            if let Some(c) = css_color(style.bg.unwrap_or(Color::Reset), false) {
                let _ = write!(css, "background:{c};");
            }
            if style.add_modifier.contains(Modifier::BOLD) {
                css.push_str("font-weight:700;");
            }
            if style.add_modifier.contains(Modifier::ITALIC) {
                css.push_str("font-style:italic;");
            }
            if style.add_modifier.contains(Modifier::UNDERLINED) {
                css.push_str("text-decoration:underline;");
            }
            if style.add_modifier.contains(Modifier::CROSSED_OUT) {
                css.push_str("text-decoration:line-through;");
            }
            let escaped = run
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            if css.is_empty() {
                html.push_str(&escaped);
            } else {
                let _ = write!(html, "<span style=\"{css}\">{escaped}</span>");
            }
            x = x2;
        }
        html.push('\n');
    }

    let page = format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><style>
html,body{{margin:0;background:transparent}}
.window{{display:inline-block;background:#1e1f22;border-radius:12px;box-shadow:0 20px 60px rgba(0,0,0,.45);overflow:hidden;font-family:Menlo,Monaco,"DejaVu Sans Mono",monospace;font-size:13px;line-height:1.35}}
.bar{{height:34px;background:#2b2d31;display:flex;align-items:center;padding:0 14px;gap:8px;color:#9da3ad;font-family:-apple-system,system-ui,sans-serif;font-size:13px}}
.bar i{{width:12px;height:12px;border-radius:50%;display:inline-block}}
.bar .t{{margin-left:auto;margin-right:auto}}
pre{{margin:0;padding:12px 16px 14px;color:#d4d4d4;white-space:pre}}
</style></head><body><div class="window"><div class="bar"><i style="background:#ff5f57"></i><i style="background:#febc2e"></i><i style="background:#28c840"></i><span class="t">contextls — {cols}×{rows}</span></div><pre>{html}</pre></div></body></html>"#
    );
    std::fs::write(&out, page).expect("write html");
    eprintln!("wrote {}", out.display());
}
