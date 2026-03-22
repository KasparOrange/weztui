use std::collections::BTreeMap;

use crate::wezterm::PaneInfo;

#[derive(Debug)]
pub struct WezWindow {
    pub window_id: u64,
    pub title: Option<String>,
    pub tabs: Vec<WezTab>,
}

#[derive(Debug)]
pub struct WezTab {
    pub tab_id: u64,
    pub title: Option<String>,
    pub panes: Vec<WezPane>,
}

#[derive(Debug)]
pub struct WezPane {
    pub pane_id: u64,
    pub title: String,
    pub cwd: Option<String>,
    pub is_active: bool,
    pub left: u64,
    pub top: u64,
    pub width: u64,
    pub height: u64,
}

/// Build a hierarchical tree from the flat pane list returned by `wezterm cli list`.
pub fn build_tree(panes: &[PaneInfo]) -> Vec<WezWindow> {
    // Group by window_id, then by tab_id (BTreeMap keeps IDs sorted)
    let mut windows: BTreeMap<u64, BTreeMap<u64, Vec<&PaneInfo>>> = BTreeMap::new();

    for pane in panes {
        windows
            .entry(pane.window_id)
            .or_default()
            .entry(pane.tab_id)
            .or_default()
            .push(pane);
    }

    windows
        .into_iter()
        .map(|(window_id, tabs)| {
            // Use the window_title from the first pane (all panes in a window share it)
            let window_title = tabs
                .values()
                .next()
                .and_then(|panes| panes.first())
                .and_then(|p| p.window_title.clone())
                .filter(|t| !t.is_empty());

            let tabs = tabs
                .into_iter()
                .map(|(tab_id, panes)| {
                    let tab_title = panes
                        .first()
                        .and_then(|p| p.tab_title.clone())
                        .filter(|t| !t.is_empty());

                    let panes = panes
                        .into_iter()
                        .map(|p| WezPane {
                            pane_id: p.pane_id,
                            title: p.title.clone(),
                            cwd: p.cwd.clone(),
                            is_active: p.is_active,
                            left: p.left,
                            top: p.top,
                            width: p.width,
                            height: p.height,
                        })
                        .collect();

                    WezTab { tab_id, title: tab_title, panes }
                })
                .collect();

            WezWindow { window_id, title: window_title, tabs }
        })
        .collect()
}
