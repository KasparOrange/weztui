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
