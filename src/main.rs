use std::io;

use color_eyre::Result;
use crossterm::event::{DisableFocusChange, EnableFocusChange};
use crossterm::execute;

mod app;
mod model;
mod ui;
mod wezterm;

fn main() -> Result<()> {
    color_eyre::install()?;

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
    let result = app::App::new(current_pane_id)?.run(&mut terminal);
    ratatui::restore();
    let _ = execute!(io::stdout(), DisableFocusChange);

    result
}
