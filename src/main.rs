#![deny(warnings)]

use std::io;

use clap::{Parser, Subcommand};
use color_eyre::Result;
use crossterm::event::{DisableFocusChange, EnableFocusChange};
use crossterm::execute;

mod app;
mod install;
mod ipc;
mod model;
mod search;
mod session;
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
    /// Save the current workspace as a named session
    Save {
        /// Session name
        name: String,
    },
    /// Restore a previously saved session
    Load {
        /// Session name
        name: String,
    },
    /// List saved sessions
    Sessions,
    /// Delete a saved session
    Delete {
        /// Session name
        name: String,
    },
    /// Install weztui keybinding into WezTerm config
    Install,
    /// Remove weztui keybinding from WezTerm config
    Uninstall,
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Save { name }) => cmd_save(&name),
        Some(Commands::Load { name }) => cmd_load(&name),
        Some(Commands::Sessions) => cmd_sessions(),
        Some(Commands::Delete { name }) => cmd_delete(&name),
        Some(Commands::Install) => install::install(),
        Some(Commands::Uninstall) => install::uninstall(),
        tui_command => {
            let current_pane_id: Option<u64> = std::env::var("WEZTERM_PANE")
                .ok()
                .and_then(|s| s.parse().ok());

            // Pre-flight check: can we talk to WezTerm?
            if let Err(e) = wezterm::list_panes() {
                eprintln!("weztui: {e}");
                std::process::exit(1);
            }

            execute!(io::stdout(), EnableFocusChange)?;
            ipc::signal_active(true);

            // Retry terminal init — when spawned via WezTerm keybinding,
            // the PTY may not be ready immediately
            let mut terminal = None;
            for _ in 0..10 {
                match ratatui::try_init() {
                    Ok(t) => { terminal = Some(t); break; }
                    Err(_) => std::thread::sleep(std::time::Duration::from_millis(50)),
                }
            }
            let mut terminal = terminal.ok_or_else(|| {
                color_eyre::eyre::eyre!("Failed to initialize terminal after retries")
            })?;

            let result = match tui_command {
                None => app::App::new(current_pane_id)?.run(&mut terminal),
                Some(Commands::Find { query }) => {
                    app::App::new_find_mode(current_pane_id, query)?.run(&mut terminal)
                }
                _ => unreachable!(),
            };

            ratatui::restore();
            ipc::signal_active(false);
            let _ = execute!(io::stdout(), DisableFocusChange);
            result
        }
    }
}

fn cmd_save(name: &str) -> Result<()> {
    let panes = wezterm::list_panes()?;
    let windows = model::build_tree(&panes);
    let sess = session::capture_session(name, &windows);
    let path = session::save_session(&sess)?;
    println!("Session '{}' saved to {}", name, path.display());
    println!(
        "  {} window(s), {} tab(s)",
        sess.windows.len(),
        sess.windows.iter().map(|w| w.tabs.len()).sum::<usize>()
    );
    Ok(())
}

fn cmd_load(name: &str) -> Result<()> {
    let sess = session::load_session(name)?;
    println!("Restoring session '{}'...", name);
    let report = session::restore_session(&sess)?;
    println!(
        "Created {} window(s), {} tab(s), {} pane(s)",
        report.windows_created, report.tabs_created, report.panes_created
    );
    if !report.errors.is_empty() {
        eprintln!("Warnings:");
        for e in &report.errors {
            eprintln!("  - {e}");
        }
    }
    Ok(())
}

fn cmd_sessions() -> Result<()> {
    let sessions = session::list_sessions()?;
    if sessions.is_empty() {
        println!("No saved sessions.");
    } else {
        for s in &sessions {
            println!(
                "{:<20} {} window(s), {} tab(s)  [saved {}]",
                s.name, s.window_count, s.tab_count, s.saved_at
            );
        }
    }
    Ok(())
}

fn cmd_delete(name: &str) -> Result<()> {
    session::delete_session(name)?;
    println!("Deleted session '{name}'");
    Ok(())
}
