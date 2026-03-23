use std::process::Command;

use color_eyre::{Result, eyre::eyre};
use serde::Deserialize;

/// Raw pane info from `wezterm cli list --format json`.
#[derive(Debug, Deserialize)]
pub struct PaneInfo {
    pub window_id: u64,
    pub tab_id: u64,
    pub tab_title: Option<String>,
    pub window_title: Option<String>,
    pub pane_id: u64,
    #[allow(dead_code)]
    pub workspace: Option<String>,
    pub title: String,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub left: u64,
    #[serde(default)]
    pub top: u64,
    #[serde(default)]
    pub width: u64,
    #[serde(default)]
    pub height: u64,
}

/// Query all panes from the running WezTerm instance.
pub fn list_panes() -> Result<Vec<PaneInfo>> {
    let output = Command::new("wezterm")
        .args(["cli", "list", "--format", "json"])
        .output()
        .map_err(|e| eyre!("Failed to run `wezterm cli list`: {e}. Is WezTerm running?"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("wezterm cli list failed: {stderr}"));
    }

    let panes: Vec<PaneInfo> = serde_json::from_slice(&output.stdout)
        .map_err(|e| eyre!("Failed to parse wezterm JSON output: {e}"))?;

    Ok(panes)
}

/// Move a pane to a new tab in the specified window.
pub fn move_pane_to_window(pane_id: u64, window_id: u64) -> Result<()> {
    let output = Command::new("wezterm")
        .args([
            "cli", "move-pane-to-new-tab",
            "--window-id", &window_id.to_string(),
            "--pane-id", &pane_id.to_string(),
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("move-pane-to-new-tab failed: {stderr}"));
    }
    Ok(())
}

/// Activate (focus) a specific pane.
pub fn activate_pane(pane_id: u64) -> Result<()> {
    let output = Command::new("wezterm")
        .args(["cli", "activate-pane", "--pane-id", &pane_id.to_string()])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("activate-pane failed: {stderr}"));
    }
    Ok(())
}

/// Set a tab's title (targets the tab containing the given pane).
pub fn set_tab_title(pane_id: u64, title: &str) -> Result<()> {
    let output = Command::new("wezterm")
        .args(["cli", "set-tab-title", "--pane-id", &pane_id.to_string(), title])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("set-tab-title failed: {stderr}"));
    }
    Ok(())
}

/// Set a window's title (targets the window containing the given pane).
pub fn set_window_title(pane_id: u64, title: &str) -> Result<()> {
    let output = Command::new("wezterm")
        .args(["cli", "set-window-title", "--pane-id", &pane_id.to_string(), title])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("set-window-title failed: {stderr}"));
    }
    Ok(())
}

/// Kill (close) a pane.
pub fn kill_pane(pane_id: u64) -> Result<()> {
    let output = Command::new("wezterm")
        .args(["cli", "kill-pane", "--pane-id", &pane_id.to_string()])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("kill-pane failed: {stderr}"));
    }
    Ok(())
}

pub enum PaneSplitDirection {
    Right,
    Bottom,
}

/// Spawn a new pane. If `window_id` is Some, creates a new tab in that window.
/// If `window_id` is None, creates a new window. Returns the new pane_id.
pub fn spawn_pane(window_id: Option<u64>, cwd: Option<&str>) -> Result<u64> {
    let mut args = vec!["cli".to_string(), "spawn".to_string()];

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

    let output = Command::new("wezterm")
        .args(&args)
        .output()
        .map_err(|e| eyre!("Failed to run `wezterm cli spawn`: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("wezterm cli spawn failed: {stderr}"));
    }

    let pane_id: u64 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .map_err(|e| eyre!("Failed to parse spawn output as pane_id: {e}"))?;

    Ok(pane_id)
}

/// Split an existing pane. Returns the pane_id of the newly created pane.
pub fn split_pane(
    pane_id: u64,
    direction: PaneSplitDirection,
    percent: Option<u16>,
    cwd: Option<&str>,
) -> Result<u64> {
    let pane_id_str = pane_id.to_string();
    let mut args = vec!["cli".to_string(), "split-pane".to_string(), "--pane-id".to_string(), pane_id_str];

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

    let output = Command::new("wezterm")
        .args(&args)
        .output()
        .map_err(|e| eyre!("Failed to run `wezterm cli split-pane`: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("wezterm cli split-pane failed: {stderr}"));
    }

    let new_pane_id: u64 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .map_err(|e| eyre!("Failed to parse split-pane output as pane_id: {e}"))?;

    Ok(new_pane_id)
}
