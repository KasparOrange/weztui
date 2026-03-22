use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use tui_tree_widget::{TreeItem, TreeState};

use crate::model::{self, WezWindow};
use crate::wezterm;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NodeId {
    Window(u64),
    Tab(u64),
    Pane(u64),
}

pub struct App {
    should_quit: bool,
    pub windows: Vec<WezWindow>,
    pub tree_state: TreeState<NodeId>,
    pub current_pane_id: Option<u64>,
}

impl App {
    pub fn new(current_pane_id: Option<u64>) -> Result<Self> {
        let panes = wezterm::list_panes()?;
        let windows = model::build_tree(&panes);

        let mut tree_state = TreeState::default();

        // Auto-expand and select the current pane
        if let Some(pane_id) = current_pane_id {
            'outer: for w in &windows {
                for t in &w.tabs {
                    for p in &t.panes {
                        if p.pane_id == pane_id {
                            tree_state.open(vec![NodeId::Window(w.window_id)]);
                            tree_state.open(vec![
                                NodeId::Window(w.window_id),
                                NodeId::Tab(t.tab_id),
                            ]);
                            tree_state.select(vec![
                                NodeId::Window(w.window_id),
                                NodeId::Tab(t.tab_id),
                                NodeId::Pane(p.pane_id),
                            ]);
                            break 'outer;
                        }
                    }
                }
            }
        } else if !windows.is_empty() {
            // Expand first window and select it
            tree_state.open(vec![NodeId::Window(windows[0].window_id)]);
            tree_state.select_first();
        }

        Ok(Self {
            should_quit: false,
            windows,
            tree_state,
            current_pane_id,
        })
    }

    pub fn run(&mut self, terminal: &mut ratatui::DefaultTerminal) -> Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| crate::ui::draw(frame, self))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn handle_events(&mut self) -> Result<()> {
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                self.handle_key(key.code);
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('j') | KeyCode::Down => { self.tree_state.key_down(); }
            KeyCode::Char('k') | KeyCode::Up => { self.tree_state.key_up(); }
            KeyCode::Char('h') | KeyCode::Left => { self.tree_state.key_left(); }
            KeyCode::Char('l') | KeyCode::Right => { self.tree_state.key_right(); }
            KeyCode::Enter => { self.tree_state.toggle_selected(); }
            KeyCode::Home => { self.tree_state.select_first(); }
            KeyCode::End => { self.tree_state.select_last(); }
            _ => {}
        }
    }

    pub fn build_tree_items(&self) -> Vec<TreeItem<'_, NodeId>> {
        build_tree_items(&self.windows, self.current_pane_id)
    }

    pub fn selected_info(&self) -> String {
        let selected = self.tree_state.selected();
        match selected.last() {
            Some(NodeId::Window(id)) => {
                if let Some(w) = self.windows.iter().find(|w| w.window_id == *id) {
                    let name = w.title.as_deref().unwrap_or("(unnamed)");
                    format!("Window {id} - {name} - {} tab(s)", w.tabs.len())
                } else {
                    format!("Window {id}")
                }
            }
            Some(NodeId::Tab(id)) => {
                for w in &self.windows {
                    if let Some(t) = w.tabs.iter().find(|t| t.tab_id == *id) {
                        let title = t.title.as_deref().unwrap_or("(unnamed)");
                        return format!("Tab {id} - {title} - {} pane(s)", t.panes.len());
                    }
                }
                format!("Tab {id}")
            }
            Some(NodeId::Pane(id)) => {
                for w in &self.windows {
                    for t in &w.tabs {
                        if let Some(p) = t.panes.iter().find(|p| p.pane_id == *id) {
                            let cwd = p.cwd.as_deref().unwrap_or("?");
                            return format!("Pane {id} - {} - {cwd}", p.title);
                        }
                    }
                }
                format!("Pane {id}")
            }
            None => "No selection".to_string(),
        }
    }
}

/// Convert model windows into TreeItem hierarchy for the tree widget.
pub fn build_tree_items<'a>(
    windows: &'a [WezWindow],
    current_pane_id: Option<u64>,
) -> Vec<TreeItem<'a, NodeId>> {
    windows
        .iter()
        .map(|window| {
            let window_label = format!(
                "{}  ({})",
                window.title.as_deref().unwrap_or(&format!("Window {}", window.window_id)),
                window.window_id,
            );

            let tab_children: Vec<TreeItem<'_, NodeId>> = window
                .tabs
                .iter()
                .map(|tab| {
                    let tab_label = format!(
                        "{}  ({} pane{})",
                        tab.title
                            .as_deref()
                            .unwrap_or(&format!("Tab {}", tab.tab_id)),
                        tab.panes.len(),
                        if tab.panes.len() == 1 { "" } else { "s" },
                    );

                    let pane_children: Vec<TreeItem<'_, NodeId>> = tab
                        .panes
                        .iter()
                        .map(|pane| {
                            let cwd_short = pane
                                .cwd
                                .as_ref()
                                .and_then(|c| c.rsplit('/').next())
                                .unwrap_or("~");
                            let marker = if Some(pane.pane_id) == current_pane_id {
                                " *"
                            } else {
                                ""
                            };
                            let pane_label =
                                format!("{} [{}]{}", pane.title, cwd_short, marker);
                            TreeItem::new_leaf(NodeId::Pane(pane.pane_id), pane_label)
                        })
                        .collect();

                    TreeItem::new(NodeId::Tab(tab.tab_id), tab_label, pane_children)
                        .expect("duplicate tab pane ids")
                })
                .collect();

            TreeItem::new(
                NodeId::Window(window.window_id),
                window_label,
                tab_children,
            )
            .expect("duplicate window tab ids")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{WezPane, WezTab, WezWindow};

    fn make_window(id: u64, title: &str, tabs: Vec<WezTab>) -> WezWindow {
        WezWindow {
            window_id: id,
            title: Some(title.to_string()),
            tabs,
        }
    }

    fn make_tab(id: u64, title: &str, panes: Vec<WezPane>) -> WezTab {
        WezTab {
            tab_id: id,
            title: Some(title.to_string()),
            panes,
        }
    }

    fn make_pane(id: u64, title: &str, cwd: &str) -> WezPane {
        WezPane {
            pane_id: id,
            title: title.to_string(),
            cwd: Some(cwd.to_string()),
            is_active: false,
            left: 0,
            top: 0,
            width: 80,
            height: 24,
        }
    }

    fn sample_windows() -> Vec<WezWindow> {
        vec![
            make_window(1, "Dev", vec![
                make_tab(10, "editor", vec![
                    make_pane(100, "nvim", "/home/user/code"),
                    make_pane(101, "zsh", "/home/user/code"),
                ]),
                make_tab(11, "logs", vec![
                    make_pane(102, "tail", "/var/log"),
                ]),
            ]),
            make_window(2, "Browser", vec![
                make_tab(20, "http", vec![
                    make_pane(200, "curl", "/tmp"),
                ]),
            ]),
        ]
    }

    // -- build_tree_items tests --

    #[test]
    fn tree_items_match_window_count() {
        let windows = sample_windows();
        let items = build_tree_items(&windows, None);
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn tree_items_empty_for_empty_windows() {
        let items = build_tree_items(&[], None);
        assert!(items.is_empty());
    }

    #[test]
    fn current_pane_marker_appears_in_label() {
        let windows = vec![
            make_window(1, "Dev", vec![
                make_tab(10, "editor", vec![
                    make_pane(100, "nvim", "/home/user/code"),
                ]),
            ]),
        ];

        // With current pane set
        let items = build_tree_items(&windows, Some(100));
        // The pane leaf should contain " *"
        let pane_text = format!("{:?}", items);
        assert!(pane_text.contains("*"), "expected current pane marker in tree items");

        // Without current pane — no marker
        let items = build_tree_items(&windows, None);
        let pane_text = format!("{:?}", items);
        assert!(!pane_text.contains("*"), "unexpected marker when no current pane");
    }

    // -- selected_info tests --

    fn app_with_selection(windows: Vec<WezWindow>, selection: Vec<NodeId>) -> App {
        let mut tree_state = TreeState::default();
        tree_state.select(selection);
        App {
            should_quit: false,
            windows,
            tree_state,
            current_pane_id: None,
        }
    }

    #[test]
    fn selected_info_for_window() {
        let app = app_with_selection(
            sample_windows(),
            vec![NodeId::Window(1)],
        );
        let info = app.selected_info();
        assert!(info.contains("Window 1"));
        assert!(info.contains("Dev"));
        assert!(info.contains("2 tab(s)"));
    }

    #[test]
    fn selected_info_for_tab() {
        let app = app_with_selection(
            sample_windows(),
            vec![NodeId::Window(1), NodeId::Tab(10)],
        );
        let info = app.selected_info();
        assert!(info.contains("Tab 10"));
        assert!(info.contains("editor"));
        assert!(info.contains("2 pane(s)"));
    }

    #[test]
    fn selected_info_for_pane() {
        let app = app_with_selection(
            sample_windows(),
            vec![NodeId::Window(1), NodeId::Tab(10), NodeId::Pane(100)],
        );
        let info = app.selected_info();
        assert!(info.contains("Pane 100"));
        assert!(info.contains("nvim"));
        assert!(info.contains("/home/user/code"));
    }

    #[test]
    fn selected_info_no_selection() {
        let app = app_with_selection(sample_windows(), vec![]);
        assert_eq!(app.selected_info(), "No selection");
    }

    // -- handle_key tests --

    fn app_with_windows(windows: Vec<WezWindow>) -> App {
        let mut tree_state = TreeState::default();
        if !windows.is_empty() {
            tree_state.open(vec![NodeId::Window(windows[0].window_id)]);
            tree_state.select_first();
        }
        App {
            should_quit: false,
            windows,
            tree_state,
            current_pane_id: None,
        }
    }

    #[test]
    fn quit_on_q() {
        let mut app = app_with_windows(sample_windows());
        assert!(!app.should_quit);
        app.handle_key(KeyCode::Char('q'));
        assert!(app.should_quit);
    }

    #[test]
    fn quit_on_esc() {
        let mut app = app_with_windows(sample_windows());
        app.handle_key(KeyCode::Esc);
        assert!(app.should_quit);
    }

    #[test]
    fn navigation_j_k_moves_selection() {
        // TreeState navigation requires render passes so it knows visible items.
        let mut app = app_with_windows(sample_windows());

        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal.draw(|frame| crate::ui::draw(frame, &mut app)).unwrap();

        // Move down twice to get away from the top
        app.handle_key(KeyCode::Char('j'));
        terminal.draw(|frame| crate::ui::draw(frame, &mut app)).unwrap();
        app.handle_key(KeyCode::Char('j'));
        terminal.draw(|frame| crate::ui::draw(frame, &mut app)).unwrap();
        let pos_after_two_j = app.tree_state.selected().to_vec();

        // Move up — should change
        app.handle_key(KeyCode::Char('k'));
        terminal.draw(|frame| crate::ui::draw(frame, &mut app)).unwrap();
        let pos_after_k = app.tree_state.selected().to_vec();

        assert_ne!(pos_after_two_j, pos_after_k, "k should move selection up");
    }

    #[test]
    fn unknown_key_is_ignored() {
        let mut app = app_with_windows(sample_windows());
        let before = app.tree_state.selected().to_vec();
        app.handle_key(KeyCode::Char('z'));
        let after = app.tree_state.selected().to_vec();
        assert_eq!(before, after);
        assert!(!app.should_quit);
    }
}
