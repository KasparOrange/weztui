use std::process::{Command, Output};
use std::sync::OnceLock;
use std::time::Instant;

use color_eyre::{Result, eyre::eyre};
use serde::Deserialize;

/// `get-text` runs once per drawn frame (the pane preview): counted, not logged per call.
static GET_TEXT: mwlog::Summary = mwlog::Summary::new();

/// Find the wezterm binary path. Checks WEZTERM_EXECUTABLE_DIR env var first
/// (set by WezTerm itself), then common locations, then falls back to "wezterm".
fn wezterm_bin() -> &'static str {
    static BIN: OnceLock<String> = OnceLock::new();
    BIN.get_or_init(|| {
        // WezTerm sets this env var pointing to its install dir
        if let Ok(dir) = std::env::var("WEZTERM_EXECUTABLE_DIR") {
            let path = format!("{dir}/wezterm");
            if std::path::Path::new(&path).exists() {
                tracing::debug!(path, from = "WEZTERM_EXECUTABLE_DIR", "wezterm binary");
                return path;
            }
        }
        // Common macOS locations
        for candidate in [
            "/Applications/WezTerm.app/Contents/MacOS/wezterm",
            "/usr/local/bin/wezterm",
            "/opt/homebrew/bin/wezterm",
        ] {
            if std::path::Path::new(candidate).exists() {
                tracing::debug!(path = candidate, from = "known location", "wezterm binary");
                return candidate.to_string();
            }
        }
        tracing::warn!("wezterm binary not found in the known places, falling back to PATH");
        "wezterm".to_string()
    })
}

/// The wezterm binary every call goes to (for `weztui doctor`).
pub fn binary() -> &'static str {
    wezterm_bin()
}

/// Run `wezterm cli <args>` — the one place weztui talks to WezTerm. Every call
/// is logged with its duration and outcome (`get-text` as a 10 s summary), and
/// the current trace id travels along as `TRACE`. A non-zero exit is an error
/// carrying WezTerm's stderr.
fn cli(args: &[&str]) -> Result<Output> {
    let sub = args.first().copied().unwrap_or("");
    let hot = sub == "get-text";
    let line = args.join(" ");

    let mut command = Command::new(wezterm_bin());
    command.arg("cli").args(args);
    if let Some(trace) = mwlog::trace::current() {
        mwlog::trace::set_env(&mut command, &trace);
    }

    let started = Instant::now();
    let output = command.output();
    let elapsed = started.elapsed();
    let ms = elapsed.as_millis() as u64;

    match output {
        Ok(out) if out.status.success() => {
            if hot {
                GET_TEXT.record(elapsed);
                mwlog::summary!(GET_TEXT, sub, "wezterm cli calls");
            } else {
                tracing::debug!(sub, args = line, ms, ok = true, bytes = out.stdout.len(), "wezterm cli");
            }
            Ok(out)
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            let code = out.status.code().unwrap_or(-1);
            if hot {
                tracing::debug!(sub, args = line, ms, ok = false, code, stderr, "wezterm cli failed");
            } else {
                tracing::warn!(sub, args = line, ms, ok = false, code, stderr, "wezterm cli failed");
            }
            Err(eyre!("wezterm cli {sub} failed: {stderr}"))
        }
        Err(e) => {
            tracing::error!(sub, args = line, ms, binary = wezterm_bin(), error = %e, "wezterm cli could not be started");
            Err(eyre!("Failed to run `wezterm cli {sub}`: {e}. Is WezTerm running?"))
        }
    }
}

/// Logs what the `get-text` summary still holds (the TUI is closing).
pub fn flush_summaries() {
    if let Some(s) = GET_TEXT.take() {
        tracing::debug!(
            n = s.n, avg_ms = s.avg_ms, max_ms = s.max_ms, total_ms = s.total_ms, secs = s.secs,
            sub = "get-text", "wezterm cli calls"
        );
    }
}

/// The pane id `spawn` / `split-pane` print.
fn parse_pane_id(sub: &str, output: &Output) -> Result<u64> {
    let text = String::from_utf8_lossy(&output.stdout);
    text.trim().parse().map_err(|e| {
        tracing::warn!(sub, output = text.trim(), error = %e, "wezterm cli printed no pane id");
        eyre!("Failed to parse {sub} output as pane_id: {e}")
    })
}

/// Raw pane info from `wezterm cli list --format json`.
#[derive(Debug, Deserialize)]
pub struct PaneInfo {
    pub window_id: u64,
    pub tab_id: u64,
    #[serde(default)]
    pub tab_title: Option<String>,
    #[serde(default)]
    pub window_title: Option<String>,
    pub pane_id: u64,
    #[serde(default)]
    pub workspace: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub left_col: u64,
    #[serde(default)]
    pub top_row: u64,
    #[serde(default)]
    pub size: PaneSize,
    #[serde(default)]
    pub tty_name: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct PaneSize {
    #[serde(default)]
    pub rows: u64,
    #[serde(default)]
    pub cols: u64,
}

impl PaneInfo {
    /// Strip `file://hostname` prefix from cwd if present.
    pub fn clean_cwd(&self) -> Option<String> {
        self.cwd.as_ref().map(|c| {
            if let Some(rest) = c.strip_prefix("file://") {
                // file://hostname/path — skip to the path
                if let Some(slash) = rest.find('/') {
                    rest[slash..].to_string()
                } else {
                    rest.to_string()
                }
            } else {
                c.clone()
            }
        })
    }

    /// The pane's controlling TTY without the `/dev/` prefix (e.g. `ttys007`),
    /// matching the `TT` column of `ps`. Used to join panes to OS processes —
    /// and through them to Claude Code sessions.
    pub fn short_tty(&self) -> Option<String> {
        self.tty_name
            .as_ref()
            .map(|tty| tty.rsplit('/').next().unwrap_or(tty).to_string())
    }
}

/// Query all panes from the running WezTerm instance.
pub fn list_panes() -> Result<Vec<PaneInfo>> {
    let output = cli(&["list", "--format", "json"])?;

    let panes: Vec<PaneInfo> = serde_json::from_slice(&output.stdout).map_err(|e| {
        tracing::error!(bytes = output.stdout.len(), error = %e, "wezterm cli list: JSON not understood");
        eyre!("Failed to parse wezterm JSON output: {e}")
    })?;

    Ok(panes)
}

/// IDs of panes that currently have focus — one per connected GUI client
/// (`wezterm cli list-clients`). With several windows/clients, each may focus a
/// different pane, so this returns a set. Returns an empty set on any failure,
/// so the live-focus marker simply disappears rather than breaking the tree.
pub fn focused_pane_ids() -> std::collections::HashSet<u64> {
    #[derive(Deserialize)]
    struct ClientInfo {
        #[serde(default)]
        focused_pane_id: Option<u64>,
    }

    let mut ids = std::collections::HashSet::new();

    let Ok(output) = cli(&["list-clients", "--format", "json"]) else {
        return ids;
    };

    match serde_json::from_slice::<Vec<ClientInfo>>(&output.stdout) {
        Ok(clients) => {
            for client in clients {
                if let Some(id) = client.focused_pane_id {
                    ids.insert(id);
                }
            }
        }
        Err(e) => tracing::warn!(error = %e, "wezterm cli list-clients: JSON not understood, no focus marker"),
    }

    ids
}

/// Move a pane to a new tab in the specified window.
pub fn move_pane_to_window(pane_id: u64, window_id: u64) -> Result<()> {
    cli(&[
        "move-pane-to-new-tab",
        "--window-id", &window_id.to_string(),
        "--pane-id", &pane_id.to_string(),
    ])?;
    Ok(())
}

/// Activate (focus) a specific pane.
pub fn activate_pane(pane_id: u64) -> Result<()> {
    cli(&["activate-pane", "--pane-id", &pane_id.to_string()])?;
    Ok(())
}

/// Set a tab's title (targets the tab containing the given pane).
pub fn set_tab_title(pane_id: u64, title: &str) -> Result<()> {
    cli(&["set-tab-title", "--pane-id", &pane_id.to_string(), title])?;
    Ok(())
}

/// Set a window's title (targets the window containing the given pane).
pub fn set_window_title(pane_id: u64, title: &str) -> Result<()> {
    cli(&["set-window-title", "--pane-id", &pane_id.to_string(), title])?;
    Ok(())
}

/// Get the visible text content of a pane.
pub fn get_pane_text(pane_id: u64) -> Result<String> {
    let output = cli(&["get-text", "--pane-id", &pane_id.to_string()])?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Kill (close) a pane.
pub fn kill_pane(pane_id: u64) -> Result<()> {
    cli(&["kill-pane", "--pane-id", &pane_id.to_string()])?;
    Ok(())
}

pub enum PaneSplitDirection {
    Right,
    Bottom,
}

/// Spawn a new pane. If `window_id` is Some, creates a new tab in that window.
/// If `window_id` is None, creates a new window. Returns the new pane_id.
pub fn spawn_pane(window_id: Option<u64>, cwd: Option<&str>) -> Result<u64> {
    let mut args = vec!["spawn".to_string()];

    if let Some(wid) = window_id {
        args.push("--window-id".to_string());
        args.push(wid.to_string());
    } else {
        args.push("--new-window".to_string());
    }

    if let Some(dir) = cwd {
        args.push("--cwd".to_string());
        args.push(dir.to_string());
    }

    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = cli(&args)?;
    parse_pane_id("spawn", &output)
}

/// Split an existing pane. Returns the pane_id of the newly created pane.
pub fn split_pane(
    pane_id: u64,
    direction: PaneSplitDirection,
    percent: Option<u16>,
    cwd: Option<&str>,
) -> Result<u64> {
    let mut args = vec!["split-pane".to_string(), "--pane-id".to_string(), pane_id.to_string()];

    match direction {
        PaneSplitDirection::Right => args.push("--right".to_string()),
        PaneSplitDirection::Bottom => args.push("--bottom".to_string()),
    }

    if let Some(pct) = percent {
        args.push("--percent".to_string());
        args.push(pct.to_string());
    }

    if let Some(dir) = cwd {
        args.push("--cwd".to_string());
        args.push(dir.to_string());
    }

    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = cli(&args)?;
    parse_pane_id("split-pane", &output)
}
