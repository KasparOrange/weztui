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
    // Geometry fields — used by visual layouts feature
    #[allow(dead_code)]
    pub left: u64,
    #[allow(dead_code)]
    pub top: u64,
    #[allow(dead_code)]
    pub width: u64,
    #[allow(dead_code)]
    pub height: u64,
}

/// Build a hierarchical tree from the flat pane list returned by `wezterm cli list`.
///
/// Groups panes by window_id then tab_id, using BTreeMap for sorted output.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pane(window_id: u64, tab_id: u64, pane_id: u64) -> PaneInfo {
        PaneInfo {
            window_id,
            tab_id,
            tab_title: Some(format!("tab-{tab_id}")),
            window_title: Some(format!("window-{window_id}")),
            pane_id,
            workspace: None,
            title: format!("pane-{pane_id}"),
            cwd: Some(format!("/home/user/project-{pane_id}")),
            is_active: false,
            left: 0,
            top: 0,
            width: 80,
            height: 24,
        }
    }

    #[test]
    fn empty_input_returns_empty_tree() {
        let tree = build_tree(&[]);
        assert!(tree.is_empty());
    }

    #[test]
    fn single_pane_creates_one_window_one_tab() {
        let panes = vec![make_pane(1, 10, 100)];
        let tree = build_tree(&panes);

        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].window_id, 1);
        assert_eq!(tree[0].title.as_deref(), Some("window-1"));
        assert_eq!(tree[0].tabs.len(), 1);
        assert_eq!(tree[0].tabs[0].tab_id, 10);
        assert_eq!(tree[0].tabs[0].title.as_deref(), Some("tab-10"));
        assert_eq!(tree[0].tabs[0].panes.len(), 1);
        assert_eq!(tree[0].tabs[0].panes[0].pane_id, 100);
    }

    #[test]
    fn multiple_panes_grouped_by_window_and_tab() {
        let panes = vec![
            make_pane(1, 10, 100),
            make_pane(1, 10, 101), // same tab, split pane
            make_pane(1, 11, 102), // same window, different tab
            make_pane(2, 20, 200), // different window
        ];
        let tree = build_tree(&panes);

        assert_eq!(tree.len(), 2);

        // Window 1
        assert_eq!(tree[0].window_id, 1);
        assert_eq!(tree[0].tabs.len(), 2);
        assert_eq!(tree[0].tabs[0].tab_id, 10);
        assert_eq!(tree[0].tabs[0].panes.len(), 2);
        assert_eq!(tree[0].tabs[1].tab_id, 11);
        assert_eq!(tree[0].tabs[1].panes.len(), 1);

        // Window 2
        assert_eq!(tree[1].window_id, 2);
        assert_eq!(tree[1].tabs.len(), 1);
        assert_eq!(tree[1].tabs[0].panes.len(), 1);
    }

    #[test]
    fn windows_and_tabs_are_sorted_by_id() {
        let panes = vec![
            make_pane(3, 30, 300),
            make_pane(1, 11, 101),
            make_pane(1, 10, 100),
            make_pane(2, 20, 200),
        ];
        let tree = build_tree(&panes);

        let window_ids: Vec<u64> = tree.iter().map(|w| w.window_id).collect();
        assert_eq!(window_ids, vec![1, 2, 3]);

        let tab_ids: Vec<u64> = tree[0].tabs.iter().map(|t| t.tab_id).collect();
        assert_eq!(tab_ids, vec![10, 11]);
    }

    #[test]
    fn empty_titles_become_none() {
        let mut pane = make_pane(1, 10, 100);
        pane.window_title = Some(String::new());
        pane.tab_title = Some(String::new());

        let tree = build_tree(&[pane]);

        assert_eq!(tree[0].title, None);
        assert_eq!(tree[0].tabs[0].title, None);
    }

    #[test]
    fn pane_fields_are_preserved() {
        let mut pane = make_pane(1, 10, 100);
        pane.cwd = Some("/tmp/test".to_string());
        pane.is_active = true;
        pane.left = 5;
        pane.top = 10;
        pane.width = 120;
        pane.height = 40;

        let tree = build_tree(&[pane]);
        let p = &tree[0].tabs[0].panes[0];

        assert_eq!(p.cwd.as_deref(), Some("/tmp/test"));
        assert!(p.is_active);
        assert_eq!(p.left, 5);
        assert_eq!(p.top, 10);
        assert_eq!(p.width, 120);
        assert_eq!(p.height, 40);
    }
}
