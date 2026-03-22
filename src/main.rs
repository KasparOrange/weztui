use std::io;

use clap::{Parser, Subcommand};
use color_eyre::Result;
use crossterm::event::{DisableFocusChange, EnableFocusChange};
use crossterm::execute;

mod app;
mod model;
mod search;
mod ui;
mod wezterm;

#[derive(Parser)]
#[command(name = "weztui", about = "TUI manager for WezTerm")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Fuzzy-find and switch to a pane
    Find {
        /// Pre-fill the search query
        query: Option<String>,
    },
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();
    let current_pane_id: Option<u64> = std::env::var("WEZTERM_PANE")
        .ok()
        .and_then(|s| s.parse().ok());

    // Pre-flight check: can we talk to WezTerm?
    if let Err(e) = wezterm::list_panes() {
        eprintln!("weztui: {e}");
        std::process::exit(1);
    }

    execute!(io::stdout(), EnableFocusChange)?;
    let mut terminal = ratatui::init();

    let result = match cli.command {
        None => app::App::new(current_pane_id)?.run(&mut terminal),
        Some(Commands::Find { query }) => {
            app::App::new_find_mode(current_pane_id, query)?.run(&mut terminal)
        }
    };

    ratatui::restore();
    let _ = execute!(io::stdout(), DisableFocusChange);

    result
}
