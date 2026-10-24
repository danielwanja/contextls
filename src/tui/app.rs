//! TUI state: inventory, tree selection, filter, preview cache.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use tui_tree_widget::{TreeItem, TreeState};

use crate::cli::Grouping;
use crate::model::{Entry, Install, Inventory, Scope, ToolReport};
use crate::output::{fmt_age_short, fmt_tokens};
use crate::scan::{ScanOptions, scan_with};
use crate::tools::schema::Definitions;

pub const SCOPES: [Scope; 5] = [
    Scope::Managed,
    Scope::User,
    Scope::Project,
    Scope::Local,
    Scope::AutoMemory,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Tree,
    Preview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    None,
    Help,
    Details,
}

pub struct App {
    pub defs: Definitions,
    pub root: PathBuf,
    pub opts: ScanOptions,
    pub inv: Inventory,
    pub grouping: Grouping,
    pub show_missing: bool,
    /// Treat not-detected tools like any other instead of collapsing them.
    pub show_all_tools: bool,
    pub focus: Focus,
    pub overlay: Overlay,
    pub tree: TreeState<String>,
    pub preview_scroll: u16,
    /// Fuzzy filter text; applied while non-empty.
    pub filter: String,
    /// True while the user is typing into the filter.
    pub filtering: bool,
    /// A file previewed that is not an entry (an import target).
    pub adhoc: Option<PathBuf>,
    pub status: Option<String>,
    pub quit: bool,
    matcher: Matcher,
    contents: HashMap<PathBuf, Result<String, String>>,
}

impl App {
    pub fn new(
        defs: Definitions,
        root: PathBuf,
        opts: ScanOptions,
        inv: Inventory,
        grouping: Grouping,
        all_tools: bool,
    ) -> Self {
        let mut app = App {
            defs,
            root,
            opts,
            inv,
            grouping,
            show_missing: false,
            show_all_tools: all_tools,
            focus: Focus::Tree,
            overlay: Overlay::None,
            tree: TreeState::default(),
            preview_scroll: 0,
            filter: String::new(),
            filtering: false,
            adhoc: None,
            status: None,
            quit: false,
            matcher: Matcher::new(Config::DEFAULT),
            contents: HashMap::new(),
        };
        app.open_all();
        app.select_first_entry();
        app
    }

    /// Seed the preview cache (used by tests, which have no files on disk).
    pub fn preload(&mut self, path: PathBuf, text: String) {
        self.contents.insert(path, Ok(text));
    }

    pub fn rescan(&mut self) {
        match scan_with(&self.defs, &self.root, &self.opts) {
            Ok(inv) => {
                self.inv = inv;
                self.contents.clear();
                self.open_all();
                self.status = Some("rescanned".into());
            }
            Err(e) => self.status = Some(format!("rescan failed: {e}")),
        }
    }

    pub fn toggle_grouping(&mut self) {
        self.grouping = match self.grouping {
            Grouping::Tool => Grouping::Scope,
            Grouping::Scope => Grouping::Tool,
        };
        self.rebuild_keeping_selection();
    }

    pub fn toggle_all_tools(&mut self) {
        self.show_all_tools = !self.show_all_tools;
        self.status = Some(
            if self.show_all_tools {
                "showing not-detected tools as regular tools"
            } else {
                "collapsing not-detected tools"
            }
            .into(),
        );
        self.rebuild_keeping_selection();
    }

    fn rebuild_keeping_selection(&mut self) {
        let selected = self.selected_index();
        self.open_all();
        match selected {
            Some((t, i)) => self.select_entry(t, i),
            None => self.select_first_entry(),
        }
    }

    /// Shown as a regular tool (not under "Not detected").
    fn active(&self, t: usize) -> bool {
        self.show_all_tools || self.inv.tools[t].active()
    }

    // ───────────────────────────────────────────────────────── selection

    /// `(tool index, entry index)` of the selected tree node, if it is a file.
    pub fn selected_index(&self) -> Option<(usize, usize)> {
        let id = self.tree.selected().last()?;
        parse_entry_id(id)
    }

    pub fn selected_entry(&self) -> Option<(&ToolReport, &Entry)> {
        let (t, i) = self.selected_index()?;
        let tool = self.inv.tools.get(t)?;
        Some((tool, tool.entries.get(i)?))
    }

    /// The tool whose header node is selected (either grouping).
    pub fn selected_tool(&self) -> Option<&ToolReport> {
        let id = self.tree.selected().last()?;
        let tool_id = id.strip_prefix("t:").or_else(|| {
            id.strip_prefix("T:")
                .and_then(|r| r.split_once(':'))
                .map(|(_, t)| t)
        })?;
        self.inv.tools.iter().find(|t| t.id == tool_id)
    }

    pub fn select_entry(&mut self, t: usize, i: usize) {
        let path = self.id_path(t, i);
        for n in 1..path.len() {
            self.tree.open(path[..n].to_vec());
        }
        self.tree.select(path);
        self.preview_scroll = 0;
        self.adhoc = None;
    }

    pub fn select_first_entry(&mut self) {
        let first_in = |active: bool| {
            self.inv.tools.iter().enumerate().find_map(|(t, tool)| {
                if self.active(t) != active {
                    return None;
                }
                tool.entries.iter().position(|e| e.exists).map(|i| (t, i))
            })
        };
        let first = first_in(true).or_else(|| first_in(false));
        match first {
            Some((t, i)) => self.select_entry(t, i),
            None => {
                self.tree.select_first();
            }
        }
    }

    /// Select the entry whose path matches, or preview it ad hoc if it is not one.
    pub fn jump_to_path(&mut self, path: &Path) {
        let found = self.inv.tools.iter().enumerate().find_map(|(t, tool)| {
            tool.entries
                .iter()
                .position(|e| e.path == path)
                .map(|i| (t, i))
        });
        match found {
            Some((t, i)) => self.select_entry(t, i),
            None if path.is_file() => {
                self.adhoc = Some(path.to_path_buf());
                self.preview_scroll = 0;
            }
            None => self.status = Some(format!("{} does not exist", path.display())),
        }
    }

    /// Tree identifier path for an entry under the current grouping. Tools that
    /// are not active sit under the "Not detected" group, always tool-first.
    fn id_path(&self, t: usize, i: usize) -> Vec<String> {
        let tool = &self.inv.tools[t];
        let scope = tool.entries[i].scope;
        if !self.active(t) {
            return vec![
                inactive_id(),
                tool_id(tool),
                scope_id(tool, scope),
                entry_id(t, i),
            ];
        }
        match self.grouping {
            Grouping::Tool => vec![tool_id(tool), scope_id(tool, scope), entry_id(t, i)],
            Grouping::Scope => vec![
                scope_group_id(scope),
                tool_in_scope_id(scope, tool),
                entry_id(t, i),
            ],
        }
    }

    /// Open every group node except "Not detected", which starts collapsed.
    fn open_all(&mut self) {
        let mut paths = Vec::new();
        for (t, tool) in self.inv.tools.iter().enumerate() {
            for i in 0..tool.entries.len() {
                let p = self.id_path(t, i);
                for n in 1..p.len() {
                    if p[..n] != [inactive_id()] {
                        paths.push(p[..n].to_vec());
                    }
                }
            }
        }
        for p in paths {
            self.tree.open(p);
        }
    }

    // ───────────────────────────────────────────────────────────── filter

    fn matches(&mut self, hay: &str) -> bool {
        if self.filter.is_empty() {
            return true;
        }
        let pattern = Pattern::parse(&self.filter, CaseMatching::Ignore, Normalization::Smart);
        let mut buf = Vec::new();
        pattern
            .score(Utf32Str::new(hay, &mut buf), &mut self.matcher)
            .is_some()
    }

    fn visible(&mut self, tool: &ToolReport, e: &Entry) -> bool {
        if !self.show_missing && !e.exists {
            return false;
        }
        if self.filter.is_empty() {
            return true;
        }
        // Fuzzy on the path, plain substring on the tool name: fuzzy across
        // several fields matches almost anything.
        if tool
            .name
            .to_lowercase()
            .contains(&self.filter.to_lowercase())
        {
            return true;
        }
        let path = e.display_path.clone();
        self.matches(&path)
    }

    // ─────────────────────────────────────────────────────────────── tree

    pub fn items(&mut self) -> Vec<TreeItem<'static, String>> {
        // Collect visibility first: `visible` needs &mut self for the matcher.
        let mut vis: Vec<Vec<bool>> = Vec::new();
        let tools = std::mem::take(&mut self.inv.tools);
        for tool in &tools {
            let v = tool.entries.iter().map(|e| self.visible(tool, e)).collect();
            vis.push(v);
        }
        self.inv.tools = tools;

        match self.grouping {
            Grouping::Tool => self.items_by_tool(&vis),
            Grouping::Scope => self.items_by_scope(&vis),
        }
    }

    /// One tool's subtree: Tool ▸ Scope ▸ entries. `None` when a filter leaves nothing.
    fn tool_item(
        &self,
        t: usize,
        tool: &ToolReport,
        vis: &[Vec<bool>],
    ) -> Option<TreeItem<'static, String>> {
        let mut scopes = Vec::new();
        let dups = dup_memory_names(tool);
        for scope in SCOPES {
            let leaves: Vec<_> = tool
                .entries
                .iter()
                .enumerate()
                .filter(|(i, e)| e.scope == scope && vis[t][*i])
                .map(|(i, e)| {
                    TreeItem::new_leaf(entry_id(t, i), entry_line(e, dups.contains(&e.file_name())))
                })
                .collect();
            if leaves.is_empty() {
                continue;
            }
            let n = leaves.len();
            scopes.push(
                TreeItem::new(scope_id(tool, scope), scope_line(scope, n), leaves)
                    .expect("unique entry ids"),
            );
        }
        if scopes.is_empty() && !self.filter.is_empty() {
            return None;
        }
        Some(
            TreeItem::new(tool_id(tool), tool_line(tool, self.active(t)), scopes)
                .expect("unique scope ids"),
        )
    }

    /// The collapsed "Not detected" group holding every inactive tool.
    fn inactive_group(&self, vis: &[Vec<bool>]) -> Option<TreeItem<'static, String>> {
        let inactive: Vec<usize> = (0..self.inv.tools.len())
            .filter(|t| !self.active(*t))
            .collect();
        if inactive.is_empty() {
            return None;
        }
        let items: Vec<_> = inactive
            .iter()
            .filter_map(|&t| self.tool_item(t, &self.inv.tools[t], vis))
            .collect();
        if items.is_empty() {
            return None;
        }
        let n = inactive.len();
        let line = Line::from(vec![
            Span::styled("Not detected", Style::new().dark_gray().bold()),
            Span::styled(
                format!(
                    "  {n} tool{} · press a to expand all",
                    if n == 1 { "" } else { "s" }
                ),
                Style::new().dark_gray().italic(),
            ),
        ]);
        Some(TreeItem::new(inactive_id(), line, items).expect("unique tool ids"))
    }

    fn items_by_tool(&self, vis: &[Vec<bool>]) -> Vec<TreeItem<'static, String>> {
        let mut out: Vec<_> = self
            .inv
            .tools
            .iter()
            .enumerate()
            .filter(|(t, _)| self.active(*t))
            .filter_map(|(t, tool)| self.tool_item(t, tool, vis))
            .collect();
        out.extend(self.inactive_group(vis));
        out
    }

    fn items_by_scope(&self, vis: &[Vec<bool>]) -> Vec<TreeItem<'static, String>> {
        let mut out = Vec::new();
        for scope in SCOPES {
            let mut tools = Vec::new();
            for (t, tool) in self.inv.tools.iter().enumerate() {
                if !self.active(t) {
                    continue;
                }
                let dups = dup_memory_names(tool);
                let leaves: Vec<_> = tool
                    .entries
                    .iter()
                    .enumerate()
                    .filter(|(i, e)| e.scope == scope && vis[t][*i])
                    .map(|(i, e)| {
                        TreeItem::new_leaf(
                            entry_id(t, i),
                            entry_line(e, dups.contains(&e.file_name())),
                        )
                    })
                    .collect();
                if leaves.is_empty() {
                    continue;
                }
                tools.push(
                    TreeItem::new(tool_in_scope_id(scope, tool), tool_line(tool, true), leaves)
                        .expect("unique entry ids"),
                );
            }
            if tools.is_empty() {
                continue;
            }
            let n: usize = tools.iter().map(|t| t.children().len()).sum();
            out.push(
                TreeItem::new(scope_group_id(scope), scope_line(scope, n), tools)
                    .expect("unique tool ids"),
            );
        }
        out.extend(self.inactive_group(vis));
        out
    }

    // ──────────────────────────────────────────────────────────── preview

    /// The file the preview pane shows.
    pub fn preview_path(&self) -> Option<PathBuf> {
        if let Some(p) = &self.adhoc {
            return Some(p.clone());
        }
        let (_, e) = self.selected_entry()?;
        e.exists.then(|| e.path.clone())
    }

    /// Load the previewed file into the cache (call before drawing).
    pub fn ensure_content(&mut self) {
        let Some(path) = self.preview_path() else {
            return;
        };
        if self.contents.contains_key(&path) {
            return;
        }
        let res = fs::read_to_string(&path).map_err(|e| e.to_string());
        self.contents.insert(path, res);
    }

    pub fn content(&self, path: &Path) -> Option<&Result<String, String>> {
        self.contents.get(path)
    }
}

// ───────────────────────────────────────────────────────── identifiers

fn inactive_id() -> String {
    "x:not-detected".to_string()
}
fn tool_id(tool: &ToolReport) -> String {
    format!("t:{}", tool.id)
}
fn scope_id(tool: &ToolReport, scope: Scope) -> String {
    format!("s:{}:{:?}", tool.id, scope)
}
fn scope_group_id(scope: Scope) -> String {
    format!("S:{scope:?}")
}
fn tool_in_scope_id(scope: Scope, tool: &ToolReport) -> String {
    format!("T:{scope:?}:{}", tool.id)
}
fn entry_id(t: usize, i: usize) -> String {
    format!("e:{t}:{i}")
}
fn parse_entry_id(id: &str) -> Option<(usize, usize)> {
    let rest = id.strip_prefix("e:")?;
    let (t, i) = rest.split_once(':')?;
    Some((t.parse().ok()?, i.parse().ok()?))
}

// ─────────────────────────────────────────────────────────────── lines

pub fn scope_color(scope: Scope) -> Color {
    match scope {
        Scope::Managed => Color::Red,
        Scope::User => Color::Green,
        Scope::Project => Color::Reset,
        Scope::Local => Color::Yellow,
        Scope::AutoMemory => Color::Magenta,
    }
}

fn tool_line(tool: &ToolReport, active: bool) -> Line<'static> {
    let n = tool.present().count();
    if !active {
        let why = match tool.install {
            Install::NotInstalled { .. } => "not installed",
            _ => "nothing loaded here",
        };
        let files = if n > 0 {
            format!(" · {n} file{}", if n == 1 { "" } else { "s" })
        } else {
            String::new()
        };
        return Line::from(vec![
            Span::styled(tool.name.clone(), Style::new().dark_gray()),
            Span::styled(format!("  {why}{files}"), Style::new().dark_gray().italic()),
        ]);
    }
    Line::from(vec![
        Span::styled(tool.name.clone(), Style::new().cyan().bold()),
        Span::styled(
            format!(
                "  {n} file{} · {} tok",
                if n == 1 { "" } else { "s" },
                fmt_tokens(tool.total_tokens())
            ),
            Style::new().dark_gray(),
        ),
    ])
}

fn scope_line(scope: Scope, n: usize) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            scope.label().to_string(),
            Style::new().fg(scope_color(scope)),
        ),
        Span::styled(format!("  {n}"), Style::new().dark_gray()),
    ])
}

/// Auto-memory file names that occur more than once within a tool, so the
/// tree can disambiguate them with their parent directory.
fn dup_memory_names(tool: &ToolReport) -> HashSet<String> {
    let mut seen = HashSet::new();
    let mut dups = HashSet::new();
    for e in tool.entries.iter().filter(|e| e.scope == Scope::AutoMemory) {
        let name = e.file_name();
        if !seen.insert(name.clone()) {
            dups.insert(name);
        }
    }
    dups
}

fn entry_line(e: &Entry, show_parent: bool) -> Line<'static> {
    let mut spans = vec![Span::styled(
        format!("{:>2} ", e.order),
        Style::new().dark_gray(),
    )];
    let name_style = if e.exists {
        Style::new().fg(scope_color(e.scope))
    } else {
        Style::new().dark_gray().crossed_out()
    };
    // Auto-memory paths are long and share one directory: show the file name,
    // plus its parent when several memories share a name (plan.md, plan.md…).
    let name = if e.scope == Scope::AutoMemory {
        let parent = e
            .path
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned());
        match (show_parent, parent) {
            (true, Some(p)) => format!("{p}/{}", e.file_name()),
            _ => e.file_name(),
        }
    } else {
        e.display_path.clone()
    };
    spans.push(Span::styled(name, name_style));
    let age = fmt_age_short(e.mtime);
    if !age.is_empty() {
        spans.push(Span::styled(format!(" {age}"), Style::new().dark_gray()));
    }
    for b in badges(e) {
        spans.push(Span::styled(
            format!(" {b}"),
            Style::new().dark_gray().italic(),
        ));
    }
    if !e.warnings.is_empty() {
        spans.push(Span::styled(" ⚠", Style::new().red()));
    }
    Line::from(spans)
}

pub fn badges(e: &Entry) -> Vec<String> {
    let mut b = Vec::new();
    if !e.exists {
        b.push("missing".to_string());
    }
    if let Some(badge) = e.load_mode.badge() {
        let badge = if badge.chars().count() > 28 {
            format!("{}…", badge.chars().take(27).collect::<String>())
        } else {
            badge
        };
        b.push(badge);
    }
    if e.legacy {
        b.push("legacy".into());
    }
    if let Some(l) = &e.label {
        b.push(l.clone());
    }
    if !e.imports.is_empty() {
        b.push(format!("↳{}", e.imports.len()));
    }
    if !e.shared_with.is_empty() {
        b.push(format!("shared×{}", e.shared_with.len()));
    }
    b
}
