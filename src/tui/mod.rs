//! Interactive terminal UI: tree of tool → scope → files on the left,
//! rendered markdown on the right.

pub mod app;
mod editor;
mod event;
pub mod ui;
pub mod watch;

use std::path::PathBuf;

use anyhow::Result;

use crate::model::Inventory;
use crate::scan::ScanOptions;
use crate::tools::schema::Definitions;

pub use app::{App, TuiOptions};

pub fn run(
    defs: Definitions,
    root: PathBuf,
    opts: ScanOptions,
    inv: Inventory,
    ui: TuiOptions,
) -> Result<()> {
    // Leave the terminal usable if we panic.
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        ratatui::restore();
        hook(info);
    }));

    let mut terminal = ratatui::init();
    let mut app = App::new(defs, root, opts, inv, ui);
    let result = event::run_loop(&mut terminal, &mut app);
    ratatui::restore();
    result
}
