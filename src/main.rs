use std::io::IsTerminal;

use anyhow::Result;
use clap::Parser;

use contextls::cli::Args;
use contextls::{output, scan, tools, tui};

fn main() -> Result<()> {
    let args = Args::parse();
    let defs = tools::registry::load()?;

    if args.list_tools {
        for t in &defs.tools {
            println!("{:<14} {}", t.id, t.name);
        }
        return Ok(());
    }
    if args.list_summarizers {
        let env = scan::Env::from_process();
        let installed = contextls::summarize::available(&defs.summarizers, &env);
        for s in &defs.summarizers {
            let mark = match installed.iter().find(|p| p.def.id == s.id) {
                Some(p) => format!("installed  {}", p.path.display()),
                None => "-".to_string(),
            };
            println!("{:<10} {:<20} {mark}", s.id, s.name);
        }
        println!(
            "\nSearched PATH plus common install dirs (~/.local/bin, Homebrew, npm/nvm/volta/pnpm/bun). \
             Missing one? Check `which <bin>` in your shell."
        );
        return Ok(());
    }

    let root = match &args.path {
        Some(p) => p.clone(),
        None => std::env::current_dir()?,
    };
    let opts = scan::ScanOptions {
        tools: args.tools.clone(),
        git: !args.no_git,
    };
    let inventory = scan::scan_with(&defs, &root, &opts)?;

    if args.json {
        println!("{}", output::json::render(&inventory)?);
    } else if args.plain || !std::io::stdout().is_terminal() {
        print!(
            "{}",
            output::table::render(&inventory, args.all, args.all_tools)
        );
    } else {
        tui::run(
            defs,
            root,
            opts,
            inventory,
            tui::TuiOptions {
                grouping: args.group,
                all_tools: args.all_tools,
                summarizer: args.summarizer.clone(),
                watch: !args.no_watch,
            },
        )?;
    }
    Ok(())
}
