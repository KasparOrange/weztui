use color_eyre::Result;

mod app;
mod model;
mod ui;
mod wezterm;

fn main() -> Result<()> {
    color_eyre::install()?;

    let current_pane_id: Option<u64> = std::env::var("WEZTERM_PANE")
        .ok()
        .and_then(|s| s.parse().ok());

    let mut terminal = ratatui::init();
    let result = app::App::new(current_pane_id)?.run(&mut terminal);
    ratatui::restore();

    result
}
