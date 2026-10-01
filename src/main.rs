#![deny(warnings)]

use std::io;

use clap::{Parser, Subcommand};
use color_eyre::Result;
use crossterm::event::{DisableFocusChange, EnableFocusChange};
use crossterm::execute;

mod app;
mod claude;
mod dirty;
mod install;
mod ipc;
mod model;
mod search;
mod session;
mod settings;
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
    /// Inspect Claude Code sessions running in WezTerm
    Claude {
        #[command(subcommand)]
        command: ClaudeCommands,
    },
}

#[derive(Subcommand)]
enum ClaudeCommands {
    /// Show uncommitted files grouped by the Claude session that last edited them
    Dirty {
        /// Output machine-readable JSON instead of a table
        #[arg(long)]
        json: bool,
        /// Repository path to analyze (defaults to the current directory)
        #[arg(long)]
        repo: Option<String>,
    },
    /// List all live Claude Code sessions joined to their WezTerm panes
    List {
        /// Output machine-readable JSON instead of a table
        #[arg(long)]
        json: bool,
        /// Also list sessions without a terminal (`claude -p` started by a
        /// program or another session): `tty` and the pane fields are null
        #[arg(long)]
        all: bool,
    },
    /// Resolve the focused WezTerm pane to its Claude Code session (exit 1 if none)
    Focused {
        /// Output machine-readable JSON instead of a table line
        #[arg(long)]
        json: bool,
    },
}

/// One live Claude Code session joined to its WezTerm pane — the stable JSON
/// row consumed by external tooling (Raycast extensions, hooks, skills).
/// Changes to this shape must be additive only.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ClaudeSessionRow {
    /// The full session UUID (also the transcript file stem).
    session_id: String,
    /// First 8 characters of the session id, for display and `claude --resume`.
    short_id: String,
    /// PID of the `claude` CLI process.
    pid: u64,
    /// The session's working directory.
    cwd: String,
    /// Basename of the working directory, for display.
    project: String,
    /// `busy`, `idle`, or `unknown`.
    status: String,
    /// Controlling TTY without the `/dev/` prefix (e.g. `ttys007`); null only
    /// for a session without a terminal (`list --all`).
    tty: Option<String>,
    /// The session's topic title as shown in the terminal tab (the pane title
    /// Claude Code sets), if the session's TTY matched a pane.
    title: Option<String>,
    /// WezTerm pane id, if the session's TTY matched a pane.
    pane_id: Option<u64>,
    /// WezTerm window id of that pane.
    window_id: Option<u64>,
    /// WezTerm workspace of that pane.
    workspace: Option<String>,
    /// Absolute path to the session's `.jsonl` transcript, if located.
    transcript_path: Option<String>,
    /// Whether the pane is focused by any connected WezTerm client.
    focused: bool,
    /// Sessions without a TTY only: the session whose process tree started
    /// this one (a `claude -p` run from its shell); null = a program did.
    #[serde(skip_serializing_if = "Option::is_none")]
    spawned_by: Option<String>,
    /// Sessions without a TTY only: the parent process's executable basename
    /// (`hark`, `zsh`); absent = orphaned (reparented to launchd).
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_command: Option<String>,
}

/// RAII guard tying the companion plugin's `weztui_active` user variable to the
/// lifetime of the TUI session. Constructing it signals `active=true`; dropping
/// it — on a normal return, an early `?`, or a panic unwind — signals
/// `active=false`, so the plugin always restores the tab bar and clears its
/// per-window state even when the TUI crashes mid-session.
struct ActiveGuard;

impl ActiveGuard {
    fn new() -> Self {
        ipc::signal_active(true);
        ActiveGuard
    }
}

impl Drop for ActiveGuard {
    fn drop(&mut self) {
        ipc::signal_active(false);
        // Give the OSC user-var sequence time to reach WezTerm before the pane
        // may close on exit.
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
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
        Some(Commands::Claude { command }) => match command {
            ClaudeCommands::Dirty { json, repo } => cmd_claude_dirty(json, repo),
            ClaudeCommands::List { json, all } => cmd_claude_list(json, all),
            ClaudeCommands::Focused { json } => cmd_claude_focused(json),
        },
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

            // RAII guard: guarantees the companion Lua plugin is told weztui is
            // no longer active on EVERY exit path — normal return, an early `?`,
            // or a panic unwind. Without it, a panic in the TUI loop skips the
            // inactive signal, leaving the plugin with the tab bar permanently
            // hidden and the toggle hotkey stuck.
            let _active_guard = ActiveGuard::new();

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
            let _ = execute!(io::stdout(), DisableFocusChange);
            // `_active_guard` drops here — or on a panic unwind / early return —
            // emitting the inactive signal after a short delay so the user var
            // reaches WezTerm before the pane may close.
            result
        }
    }
}

fn cmd_save(name: &str) -> Result<()> {
    let panes = wezterm::list_panes()?;
    let windows = model::build_tree(&panes, &std::collections::HashMap::new());
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

/// Build the joined session rows: Claude discovery crossed with WezTerm panes
/// and client focus. WezTerm being unavailable degrades gracefully — sessions
/// still list, with the pane fields empty and `focused` false.
fn claude_session_rows(all: bool) -> Vec<ClaudeSessionRow> {
    let sessions = claude::discover();

    let panes = wezterm::list_panes().unwrap_or_default();

    let focused_ids = wezterm::focused_pane_ids();

    let mut rows: Vec<ClaudeSessionRow> = sessions
        .into_iter()
        .map(|(tty, session)| {
            let pane = panes
                .iter()
                .find(|p| p.short_tty().as_deref() == Some(tty.as_str()));

            let pane_id = pane.map(|p| p.pane_id);

            let status = match session.status {
                claude::ClaudeStatus::Busy => "busy",
                claude::ClaudeStatus::Idle => "idle",
                claude::ClaudeStatus::Unknown => "unknown",
            };

            ClaudeSessionRow {
                short_id: session.short_id().to_string(),
                pid: session.pid,
                project: std::path::Path::new(&session.cwd)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                status: status.to_string(),
                title: pane.map(|p| p.title.clone()).filter(|t| !t.is_empty()),
                pane_id,
                window_id: pane.map(|p| p.window_id),
                workspace: pane.and_then(|p| p.workspace.clone()),
                transcript_path: session.transcript_path.map(|p| p.display().to_string()),
                focused: pane_id.is_some_and(|id| focused_ids.contains(&id)),
                spawned_by: None,
                parent_command: None,
                session_id: session.session_id,
                cwd: session.cwd,
                tty: Some(tty),
            }
        })
        .collect();

    if all {
        rows.extend(claude::discover_ttyless().into_iter().map(|session| {
            let status = match session.status {
                claude::ClaudeStatus::Busy => "busy",
                claude::ClaudeStatus::Idle => "idle",
                claude::ClaudeStatus::Unknown => "unknown",
            };

            ClaudeSessionRow {
                short_id: session.short_id().to_string(),
                pid: session.pid,
                project: std::path::Path::new(&session.cwd)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                status: status.to_string(),
                title: None,
                pane_id: None,
                window_id: None,
                workspace: None,
                transcript_path: session.transcript_path.map(|p| p.display().to_string()),
                focused: false,
                spawned_by: session.spawned_by,
                parent_command: session.parent_command,
                session_id: session.session_id,
                cwd: session.cwd,
                tty: None,
            }
        }));
    }

    rows.sort_by(|a, b| {
        b.focused
            .cmp(&a.focused)
            .then_with(|| a.project.cmp(&b.project))
            .then_with(|| a.short_id.cmp(&b.short_id))
    });

    rows
}

/// Render session rows as a human-readable table (the non-`--json` view).
fn print_claude_session_table(rows: &[ClaudeSessionRow]) {
    println!(
        "{:<2} {:<8} {:<7} {:<24} {:<6} {:<8} {:<7}",
        "F", "SESSION", "STATUS", "PROJECT", "PANE", "TTY", "PID",
    );

    for row in rows {
        println!(
            "{:<2} {:<8} {:<7} {:<24} {:<6} {:<8} {:<7}",
            if row.focused { "◄" } else { "" },
            row.short_id,
            row.status,
            row.project,
            row.pane_id.map(|id| id.to_string()).unwrap_or_default(),
            row.tty.as_deref().unwrap_or(""),
            row.pid,
        );
    }
}

fn cmd_claude_list(json: bool, all: bool) -> Result<()> {
    let rows = claude_session_rows(all);

    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else if rows.is_empty() {
        println!("No live Claude Code sessions.");
    } else {
        print_claude_session_table(&rows);
    }

    Ok(())
}

fn cmd_claude_focused(json: bool) -> Result<()> {
    let rows = claude_session_rows(false);

    let Some(focused) = rows.into_iter().find(|row| row.focused) else {
        eprintln!("No focused Claude Code session.");
        std::process::exit(1);
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&focused)?);
    } else {
        print_claude_session_table(std::slice::from_ref(&focused));
    }

    Ok(())
}

fn cmd_claude_dirty(json: bool, repo: Option<String>) -> Result<()> {
    let repo = repo
        .map(std::path::PathBuf::from)
        .unwrap_or(std::env::current_dir()?);

    let report = dirty::analyze(&repo)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        dirty::print_table(&report);
    }

    Ok(())
}
