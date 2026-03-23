use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use color_eyre::{Result, eyre::eyre};
use serde::{Deserialize, Serialize};

use crate::model::{WezPane, WezWindow};
use crate::wezterm;

// -- Data model --

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Session {
    pub name: String,
    pub saved_at: String,
    pub windows: Vec<SessionWindow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionWindow {
    pub title: Option<String>,
    pub tabs: Vec<SessionTab>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionTab {
    pub title: Option<String>,
    pub root: SplitNode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum SplitNode {
    Pane {
        cwd: Option<String>,
        title: String,
    },
    Split {
        direction: SplitDirection,
        percent: u16,
        first: Box<SplitNode>,
        second: Box<SplitNode>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

#[derive(Debug)]
pub struct SessionSummary {
    pub name: String,
    pub saved_at: String,
    pub window_count: usize,
    pub tab_count: usize,
}

#[derive(Debug)]
pub struct RestoreReport {
    pub windows_created: usize,
    pub tabs_created: usize,
    pub panes_created: usize,
    pub errors: Vec<String>,
}

// -- Split reconstruction --

struct BBox {
    left: u64,
    top: u64,
    right: u64,
    bottom: u64,
}

fn bbox_of(panes: &[&WezPane]) -> BBox {
    BBox {
        left: panes.iter().map(|p| p.left).min().unwrap_or(0),
        top: panes.iter().map(|p| p.top).min().unwrap_or(0),
        right: panes.iter().map(|p| p.left + p.width).max().unwrap_or(0),
        bottom: panes.iter().map(|p| p.top + p.height).max().unwrap_or(0),
    }
}

/// Reconstruct a binary split tree from a flat list of panes with geometry.
pub fn reconstruct_splits(panes: &[WezPane]) -> SplitNode {
    let refs: Vec<&WezPane> = panes.iter().collect();
    reconstruct_recursive(&refs)
}

fn reconstruct_recursive(panes: &[&WezPane]) -> SplitNode {
    if panes.len() == 1 {
        return SplitNode::Pane {
            cwd: panes[0].cwd.clone(),
            title: panes[0].title.clone(),
        };
    }
    if panes.is_empty() {
        return SplitNode::Pane {
            cwd: None,
            title: String::new(),
        };
    }

    let bb = bbox_of(panes);
    let total_w = bb.right.saturating_sub(bb.left);
    let total_h = bb.bottom.saturating_sub(bb.top);

    // Try vertical split (left/right)
    if let Some((cut, left_idx, right_idx)) = find_vertical_cut(panes, &bb) {
        let percent = if total_w > 0 {
            ((cut.saturating_sub(bb.left)) * 100 / total_w) as u16
        } else {
            50
        };
        let left_panes: Vec<&WezPane> = left_idx.iter().map(|&i| panes[i]).collect();
        let right_panes: Vec<&WezPane> = right_idx.iter().map(|&i| panes[i]).collect();
        return SplitNode::Split {
            direction: SplitDirection::Vertical,
            percent: percent.max(1).min(99),
            first: Box::new(reconstruct_recursive(&left_panes)),
            second: Box::new(reconstruct_recursive(&right_panes)),
        };
    }

    // Try horizontal split (top/bottom)
    if let Some((cut, top_idx, bottom_idx)) = find_horizontal_cut(panes, &bb) {
        let percent = if total_h > 0 {
            ((cut.saturating_sub(bb.top)) * 100 / total_h) as u16
        } else {
            50
        };
        let top_panes: Vec<&WezPane> = top_idx.iter().map(|&i| panes[i]).collect();
        let bottom_panes: Vec<&WezPane> = bottom_idx.iter().map(|&i| panes[i]).collect();
        return SplitNode::Split {
            direction: SplitDirection::Horizontal,
            percent: percent.max(1).min(99),
            first: Box::new(reconstruct_recursive(&top_panes)),
            second: Box::new(reconstruct_recursive(&bottom_panes)),
        };
    }

    // Fallback: treat all as a flat vertical chain
    let first = panes[0];
    let rest: Vec<&WezPane> = panes[1..].to_vec();
    SplitNode::Split {
        direction: SplitDirection::Vertical,
        percent: 50,
        first: Box::new(SplitNode::Pane {
            cwd: first.cwd.clone(),
            title: first.title.clone(),
        }),
        second: Box::new(reconstruct_recursive(&rest)),
    }
}

/// Find a vertical cut line that cleanly partitions panes into left and right groups.
fn find_vertical_cut(panes: &[&WezPane], bb: &BBox) -> Option<(u64, Vec<usize>, Vec<usize>)> {
    // Collect candidate cut positions (right edges of panes, excluding the overall right edge)
    let mut candidates: Vec<u64> = panes
        .iter()
        .map(|p| p.left + p.width)
        .filter(|&r| r < bb.right)
        .collect();
    candidates.sort();
    candidates.dedup();

    for cut in candidates {
        let mut left_idx = Vec::new();
        let mut right_idx = Vec::new();
        let mut valid = true;

        for (i, p) in panes.iter().enumerate() {
            let p_right = p.left + p.width;
            if p_right <= cut + 2 && p.left < cut {
                left_idx.push(i);
            } else if p.left + 2 >= cut && p_right > cut {
                right_idx.push(i);
            } else {
                valid = false;
                break;
            }
        }

        if valid && !left_idx.is_empty() && !right_idx.is_empty() {
            return Some((cut, left_idx, right_idx));
        }
    }
    None
}

/// Find a horizontal cut line that cleanly partitions panes into top and bottom groups.
fn find_horizontal_cut(panes: &[&WezPane], bb: &BBox) -> Option<(u64, Vec<usize>, Vec<usize>)> {
    let mut candidates: Vec<u64> = panes
        .iter()
        .map(|p| p.top + p.height)
        .filter(|&b| b < bb.bottom)
        .collect();
    candidates.sort();
    candidates.dedup();

    for cut in candidates {
        let mut top_idx = Vec::new();
        let mut bottom_idx = Vec::new();
        let mut valid = true;

        for (i, p) in panes.iter().enumerate() {
            let p_bottom = p.top + p.height;
            if p_bottom <= cut + 2 && p.top < cut {
                top_idx.push(i);
            } else if p.top + 2 >= cut && p_bottom > cut {
                bottom_idx.push(i);
            } else {
                valid = false;
                break;
            }
        }

        if valid && !top_idx.is_empty() && !bottom_idx.is_empty() {
            return Some((cut, top_idx, bottom_idx));
        }
    }
    None
}

// -- Capture --

pub fn capture_session(name: &str, windows: &[WezWindow]) -> Session {
    let saved_at = {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        // Simple ISO 8601 without chrono
        let (s, m, h) = (secs % 60, (secs / 60) % 60, (secs / 3600) % 24);
        let days = secs / 86400;
        // Approximate date from days since epoch (good enough for display)
        let (y, mo, d) = days_to_ymd(days);
        format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
    };

    let session_windows = windows
        .iter()
        .map(|w| SessionWindow {
            title: w.title.clone(),
            tabs: w
                .tabs
                .iter()
                .map(|t| SessionTab {
                    title: t.title.clone(),
                    root: reconstruct_splits(&t.panes),
                })
                .collect(),
        })
        .collect();

    Session {
        name: name.to_string(),
        saved_at,
        windows: session_windows,
    }
}

fn days_to_ymd(days_since_epoch: u64) -> (u64, u64, u64) {
    // Simplified date calculation from days since 1970-01-01
    let mut y = 1970;
    let mut remaining = days_since_epoch;
    loop {
        let days_in_year = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
            366
        } else {
            365
        };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let days_in_months = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut mo = 0;
    for &dim in &days_in_months {
        if remaining < dim {
            break;
        }
        remaining -= dim;
        mo += 1;
    }
    (y, mo + 1, remaining + 1)
}

// -- File I/O --

fn sessions_dir() -> Result<PathBuf> {
    let dir = dirs_fallback().join("weztui").join("sessions");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn dirs_fallback() -> PathBuf {
    std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join(".config")
        })
}

fn session_path(name: &str) -> Result<PathBuf> {
    validate_session_name(name)?;
    Ok(sessions_dir()?.join(format!("{name}.json")))
}

fn validate_session_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(eyre!("Session name cannot be empty"));
    }
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err(eyre!(
            "Session name must contain only alphanumeric characters, hyphens, underscores, or dots"
        ));
    }
    Ok(())
}

pub fn save_session(session: &Session) -> Result<PathBuf> {
    let path = session_path(&session.name)?;
    let json = serde_json::to_string_pretty(session)?;
    fs::write(&path, json)?;
    Ok(path)
}

pub fn load_session(name: &str) -> Result<Session> {
    let path = session_path(name)?;
    if !path.exists() {
        return Err(eyre!("Session '{}' not found", name));
    }
    let json = fs::read_to_string(&path)?;
    let session: Session = serde_json::from_str(&json)?;
    Ok(session)
}

pub fn list_sessions() -> Result<Vec<SessionSummary>> {
    let dir = sessions_dir()?;
    let mut sessions = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "json") {
            if let Ok(json) = fs::read_to_string(&path) {
                if let Ok(session) = serde_json::from_str::<Session>(&json) {
                    let tab_count: usize =
                        session.windows.iter().map(|w| w.tabs.len()).sum();
                    sessions.push(SessionSummary {
                        name: session.name,
                        saved_at: session.saved_at,
                        window_count: session.windows.len(),
                        tab_count,
                    });
                }
            }
        }
    }
    sessions.sort_by(|a, b| b.saved_at.cmp(&a.saved_at));
    Ok(sessions)
}

pub fn delete_session(name: &str) -> Result<()> {
    let path = session_path(name)?;
    if !path.exists() {
        return Err(eyre!("Session '{}' not found", name));
    }
    fs::remove_file(&path)?;
    Ok(())
}

// -- Restore --

pub fn restore_session(session: &Session) -> Result<RestoreReport> {
    let mut report = RestoreReport {
        windows_created: 0,
        tabs_created: 0,
        panes_created: 0,
        errors: Vec::new(),
    };

    for (wi, window) in session.windows.iter().enumerate() {
        if window.tabs.is_empty() {
            continue;
        }

        // First tab's root pane determines the initial spawn
        let first_cwd = first_pane_cwd(&window.tabs[0].root);

        // Spawn new window
        let first_pane_id = match wezterm::spawn_pane(None, first_cwd.as_deref()) {
            Ok(id) => id,
            Err(e) => {
                report
                    .errors
                    .push(format!("Window {wi}: spawn failed: {e}"));
                continue;
            }
        };
        report.windows_created += 1;
        report.panes_created += 1;

        // Discover window_id for this new pane
        let window_id = match discover_window_id(first_pane_id) {
            Some(id) => id,
            None => {
                report
                    .errors
                    .push(format!("Window {wi}: could not discover window_id"));
                continue;
            }
        };

        // Set window title
        if let Some(ref title) = window.title {
            if let Err(e) = wezterm::set_window_title(first_pane_id, title) {
                report
                    .errors
                    .push(format!("Window {wi}: set title failed: {e}"));
            }
        }

        // Restore splits for first tab
        let pane_count = restore_split_tree(first_pane_id, &window.tabs[0].root, &mut report);
        report.panes_created += pane_count;
        report.tabs_created += 1;

        // Set first tab title
        if let Some(ref title) = window.tabs[0].title {
            if let Err(e) = wezterm::set_tab_title(first_pane_id, title) {
                report
                    .errors
                    .push(format!("Tab 0: set title failed: {e}"));
            }
        }

        // Additional tabs
        for (ti, tab) in window.tabs.iter().enumerate().skip(1) {
            let tab_cwd = first_pane_cwd(&tab.root);
            let tab_pane_id = match wezterm::spawn_pane(Some(window_id), tab_cwd.as_deref()) {
                Ok(id) => id,
                Err(e) => {
                    report
                        .errors
                        .push(format!("Tab {ti}: spawn failed: {e}"));
                    continue;
                }
            };
            report.panes_created += 1;
            report.tabs_created += 1;

            let pane_count = restore_split_tree(tab_pane_id, &tab.root, &mut report);
            report.panes_created += pane_count;

            if let Some(ref title) = tab.title {
                if let Err(e) = wezterm::set_tab_title(tab_pane_id, title) {
                    report
                        .errors
                        .push(format!("Tab {ti}: set title failed: {e}"));
                }
            }
        }
    }

    Ok(report)
}

fn first_pane_cwd(node: &SplitNode) -> Option<String> {
    match node {
        SplitNode::Pane { cwd, .. } => cwd.clone(),
        SplitNode::Split { first, .. } => first_pane_cwd(first),
    }
}

fn discover_window_id(pane_id: u64) -> Option<u64> {
    let panes = wezterm::list_panes().ok()?;
    panes.iter().find(|p| p.pane_id == pane_id).map(|p| p.window_id)
}

/// Restore a split tree into an existing root pane. Returns number of additional panes created.
fn restore_split_tree(root_pane_id: u64, node: &SplitNode, report: &mut RestoreReport) -> usize {
    match node {
        SplitNode::Pane { .. } => 0, // root pane already exists
        SplitNode::Split {
            direction,
            percent,
            first,
            second,
        } => {
            let wez_dir = match direction {
                SplitDirection::Vertical => wezterm::PaneSplitDirection::Right,
                SplitDirection::Horizontal => wezterm::PaneSplitDirection::Bottom,
            };
            let second_percent = 100u16.saturating_sub(*percent);
            let second_cwd = first_pane_cwd(second);

            let mut created = 0;

            match wezterm::split_pane(
                root_pane_id,
                wez_dir,
                Some(second_percent),
                second_cwd.as_deref(),
            ) {
                Ok(new_pane_id) => {
                    created += 1;
                    // Recurse: first child keeps root_pane_id, second child gets new_pane_id
                    created += restore_split_tree(root_pane_id, first, report);
                    created += restore_split_tree(new_pane_id, second, report);
                }
                Err(e) => {
                    report.errors.push(format!("split failed: {e}"));
                }
            }
            created
        }
    }
}

// -- Tests --

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::WezPane;

    fn pane_at(id: u64, title: &str, cwd: &str, left: u64, top: u64, w: u64, h: u64) -> WezPane {
        WezPane {
            pane_id: id,
            title: title.to_string(),
            cwd: Some(cwd.to_string()),
            is_active: false,
            left,
            top,
            width: w,
            height: h,
        }
    }

    fn count_leaves(node: &SplitNode) -> usize {
        match node {
            SplitNode::Pane { .. } => 1,
            SplitNode::Split { first, second, .. } => count_leaves(first) + count_leaves(second),
        }
    }

    #[test]
    fn single_pane_returns_leaf() {
        let panes = vec![pane_at(1, "nvim", "/code", 0, 0, 80, 24)];
        let tree = reconstruct_splits(&panes);
        assert!(matches!(tree, SplitNode::Pane { .. }));
        if let SplitNode::Pane { title, cwd } = &tree {
            assert_eq!(title, "nvim");
            assert_eq!(cwd.as_deref(), Some("/code"));
        }
    }

    #[test]
    fn vertical_split_detected() {
        let panes = vec![
            pane_at(1, "left", "/a", 0, 0, 40, 24),
            pane_at(2, "right", "/b", 41, 0, 39, 24),
        ];
        let tree = reconstruct_splits(&panes);
        match &tree {
            SplitNode::Split {
                direction: SplitDirection::Vertical,
                ..
            } => {}
            other => panic!("expected vertical split, got {other:?}"),
        }
        assert_eq!(count_leaves(&tree), 2);
    }

    #[test]
    fn horizontal_split_detected() {
        let panes = vec![
            pane_at(1, "top", "/a", 0, 0, 80, 12),
            pane_at(2, "bottom", "/b", 0, 13, 80, 11),
        ];
        let tree = reconstruct_splits(&panes);
        match &tree {
            SplitNode::Split {
                direction: SplitDirection::Horizontal,
                ..
            } => {}
            other => panic!("expected horizontal split, got {other:?}"),
        }
        assert_eq!(count_leaves(&tree), 2);
    }

    #[test]
    fn l_shape_three_panes() {
        let panes = vec![
            pane_at(1, "nvim", "/c", 0, 0, 60, 24),
            pane_at(2, "tests", "/c", 61, 0, 19, 12),
            pane_at(3, "logs", "/l", 61, 13, 19, 11),
        ];
        let tree = reconstruct_splits(&panes);
        assert_eq!(count_leaves(&tree), 3);
        match &tree {
            SplitNode::Split {
                direction: SplitDirection::Vertical,
                second,
                ..
            } => match second.as_ref() {
                SplitNode::Split {
                    direction: SplitDirection::Horizontal,
                    ..
                } => {}
                other => panic!("expected horizontal sub-split, got {other:?}"),
            },
            other => panic!("expected vertical split, got {other:?}"),
        }
    }

    #[test]
    fn inverted_l_shape() {
        let panes = vec![
            pane_at(1, "a", "/", 0, 0, 40, 12),
            pane_at(2, "b", "/", 0, 13, 40, 11),
            pane_at(3, "c", "/", 41, 0, 39, 24),
        ];
        let tree = reconstruct_splits(&panes);
        assert_eq!(count_leaves(&tree), 3);
        match &tree {
            SplitNode::Split {
                direction: SplitDirection::Vertical,
                first,
                ..
            } => match first.as_ref() {
                SplitNode::Split {
                    direction: SplitDirection::Horizontal,
                    ..
                } => {}
                other => panic!("expected horizontal sub-split on left, got {other:?}"),
            },
            other => panic!("expected vertical split, got {other:?}"),
        }
    }

    #[test]
    fn four_panes_grid() {
        let panes = vec![
            pane_at(1, "a", "/", 0, 0, 40, 12),
            pane_at(2, "b", "/", 41, 0, 39, 12),
            pane_at(3, "c", "/", 0, 13, 40, 11),
            pane_at(4, "d", "/", 41, 13, 39, 11),
        ];
        let tree = reconstruct_splits(&panes);
        assert_eq!(count_leaves(&tree), 4);
    }

    #[test]
    fn unequal_split_percent() {
        let panes = vec![
            pane_at(1, "big", "/a", 0, 0, 56, 24),
            pane_at(2, "small", "/b", 57, 0, 23, 24),
        ];
        let tree = reconstruct_splits(&panes);
        if let SplitNode::Split { percent, .. } = &tree {
            assert!(*percent >= 65 && *percent <= 75, "expected ~70%, got {percent}%");
        } else {
            panic!("expected split");
        }
    }

    #[test]
    fn leaf_count_always_matches_input() {
        let panes = vec![
            pane_at(1, "a", "/", 0, 0, 40, 12),
            pane_at(2, "b", "/", 41, 0, 39, 12),
            pane_at(3, "c", "/", 0, 13, 40, 11),
            pane_at(4, "d", "/", 41, 13, 39, 11),
        ];
        let tree = reconstruct_splits(&panes);
        assert_eq!(count_leaves(&tree), panes.len());
    }

    #[test]
    fn session_save_load_roundtrip() {
        let session = Session {
            name: "test".to_string(),
            saved_at: "2026-03-23T00:00:00Z".to_string(),
            windows: vec![SessionWindow {
                title: Some("Dev".to_string()),
                tabs: vec![SessionTab {
                    title: Some("editor".to_string()),
                    root: SplitNode::Pane {
                        cwd: Some("/tmp".to_string()),
                        title: "nvim".to_string(),
                    },
                }],
            }],
        };

        // Use a temp dir to avoid polluting real config
        let tmp = std::env::temp_dir().join("weztui-test-sessions");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let path = tmp.join("test.json");
        let json = serde_json::to_string_pretty(&session).unwrap();
        fs::write(&path, &json).unwrap();

        let loaded: Session = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(session, loaded);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn validate_session_name_rejects_bad_names() {
        assert!(validate_session_name("").is_err());
        assert!(validate_session_name("../evil").is_err());
        assert!(validate_session_name("name with spaces").is_err());
        assert!(validate_session_name("good-name_v2.0").is_ok());
        assert!(validate_session_name("simple").is_ok());
    }

    #[test]
    fn first_pane_cwd_extracts_from_tree() {
        let tree = SplitNode::Split {
            direction: SplitDirection::Vertical,
            percent: 50,
            first: Box::new(SplitNode::Pane {
                cwd: Some("/first".to_string()),
                title: "a".to_string(),
            }),
            second: Box::new(SplitNode::Pane {
                cwd: Some("/second".to_string()),
                title: "b".to_string(),
            }),
        };
        assert_eq!(first_pane_cwd(&tree), Some("/first".to_string()));
    }
}
