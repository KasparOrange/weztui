use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use tui_tree_widget::{TreeItem, TreeState};

use crate::model::{self, WezTab, WezWindow};
use crate::search::{self, SearchEntry, SearchResult};
use crate::wezterm;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NodeId {
    Window(u64),
    Tab(u64),
    Pane(u64),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Normal,
    Rename { input: String, cursor: usize },
    Move { window_choices: Vec<(u64, String)>, selected_index: usize },
    Confirm { action: PendingAction, label: String },
    Search {
        query: String,
        cursor: usize,
        entries: Vec<SearchEntry>,
        results: Vec<SearchResult>,
        selected_index: usize,
        direct_launch: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum PendingAction {
    ClosePanes(Vec<u64>),
}

#[derive(Debug, Clone)]
pub struct StatusMessage {
    pub text: String,
    pub is_error: bool,
}

pub struct App {
    pub should_quit: bool,
    pub windows: Vec<WezWindow>,
    pub tree_state: TreeState<NodeId>,
    pub current_pane_id: Option<u64>,
    pub mode: Mode,
    pub status_message: Option<StatusMessage>,
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
            tree_state.open(vec![NodeId::Window(windows[0].window_id)]);
            tree_state.select_first();
        }

        Ok(Self {
            should_quit: false,
            windows,
            tree_state,
            current_pane_id,
            mode: Mode::Normal,
            status_message: None,
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
                self.handle_key(key);
            }
            Event::FocusGained => {
                self.refresh_data();
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) {
        let code = key.code;
        // Clear status message on any keypress in Normal mode
        if self.mode == Mode::Normal {
            self.status_message = None;
        }

        match self.mode {
            Mode::Normal => self.handle_key_normal(code),
            Mode::Rename { .. } => self.handle_key_rename(code),
            Mode::Move { .. } => self.handle_key_move(code),
            Mode::Confirm { .. } => self.handle_key_confirm(code),
            Mode::Search { .. } => self.handle_key_search(key),
        }
    }

    fn handle_key_normal(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('j') | KeyCode::Down => { self.tree_state.key_down(); }
            KeyCode::Char('k') | KeyCode::Up => { self.tree_state.key_up(); }
            KeyCode::Char('h') | KeyCode::Left => { self.tree_state.key_left(); }
            KeyCode::Char('l') | KeyCode::Right => { self.tree_state.key_right(); }
            KeyCode::Enter | KeyCode::Char('f') => self.action_focus(),
            KeyCode::Home => { self.tree_state.select_first(); }
            KeyCode::End => { self.tree_state.select_last(); }
            KeyCode::Char('/') => self.enter_search_mode(false, String::new()),
            KeyCode::Char('r') => self.enter_rename_mode(),
            KeyCode::Char('m') => self.enter_move_mode(),
            KeyCode::Char('x') => self.enter_confirm_close(),
            _ => {}
        }
    }

    fn handle_key_rename(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
            }
            KeyCode::Enter => {
                self.execute_rename();
            }
            KeyCode::Backspace => {
                if let Mode::Rename { input, cursor } = &mut self.mode {
                    if *cursor > 0 {
                        input.remove(*cursor - 1);
                        *cursor -= 1;
                    }
                }
            }
            KeyCode::Delete => {
                if let Mode::Rename { input, cursor } = &mut self.mode {
                    if *cursor < input.len() {
                        input.remove(*cursor);
                    }
                }
            }
            KeyCode::Left => {
                if let Mode::Rename { cursor, .. } = &mut self.mode {
                    *cursor = cursor.saturating_sub(1);
                }
            }
            KeyCode::Right => {
                if let Mode::Rename { input, cursor } = &mut self.mode {
                    if *cursor < input.len() {
                        *cursor += 1;
                    }
                }
            }
            KeyCode::Home => {
                if let Mode::Rename { cursor, .. } = &mut self.mode {
                    *cursor = 0;
                }
            }
            KeyCode::End => {
                if let Mode::Rename { input, cursor } = &mut self.mode {
                    *cursor = input.len();
                }
            }
            KeyCode::Char(c) => {
                if let Mode::Rename { input, cursor } = &mut self.mode {
                    input.insert(*cursor, c);
                    *cursor += 1;
                }
            }
            _ => {}
        }
    }

    fn handle_key_move(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if let Mode::Move { window_choices, selected_index } = &mut self.mode {
                    if *selected_index + 1 < window_choices.len() {
                        *selected_index += 1;
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Mode::Move { selected_index, .. } = &mut self.mode {
                    *selected_index = selected_index.saturating_sub(1);
                }
            }
            KeyCode::Enter => {
                self.execute_move();
            }
            _ => {}
        }
    }

    fn handle_key_confirm(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('y') | KeyCode::Enter => {
                self.execute_close();
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.mode = Mode::Normal;
            }
            _ => {}
        }
    }

    // -- Mode entry --

    fn action_focus(&mut self) {
        match self.tree_state.selected().last() {
            Some(NodeId::Pane(pane_id)) => {
                let pane_id = *pane_id;
                match wezterm::activate_pane(pane_id) {
                    Ok(()) => self.should_quit = true,
                    Err(e) => self.set_error(format!("Focus failed: {e}")),
                }
            }
            Some(_) => {
                // Window or Tab: toggle expand/collapse
                self.tree_state.toggle_selected();
            }
            None => {}
        }
    }

    fn enter_rename_mode(&mut self) {
        match self.tree_state.selected().last() {
            Some(NodeId::Window(id)) => {
                let current_title = self.windows.iter()
                    .find(|w| w.window_id == *id)
                    .and_then(|w| w.title.clone())
                    .unwrap_or_default();
                let len = current_title.len();
                self.mode = Mode::Rename { input: current_title, cursor: len };
            }
            Some(NodeId::Tab(id)) => {
                let current_title = self.find_tab(*id)
                    .and_then(|t| t.title.clone())
                    .unwrap_or_default();
                let len = current_title.len();
                self.mode = Mode::Rename { input: current_title, cursor: len };
            }
            Some(NodeId::Pane(_)) => {
                self.set_error("Cannot rename panes — select a tab or window".into());
            }
            None => {}
        }
    }

    fn enter_move_mode(&mut self) {
        match self.tree_state.selected().last() {
            Some(NodeId::Pane(pane_id)) => {
                let pane_id = *pane_id;
                let current_window_id = self.find_pane_window(pane_id);

                let choices: Vec<(u64, String)> = self.windows.iter()
                    .filter(|w| Some(w.window_id) != current_window_id)
                    .map(|w| {
                        let label = w.title.as_deref().unwrap_or("(unnamed)");
                        (w.window_id, format!("Window {} — {}", w.window_id, label))
                    })
                    .collect();

                if choices.is_empty() {
                    self.set_error("No other windows to move to".into());
                } else {
                    self.mode = Mode::Move { window_choices: choices, selected_index: 0 };
                }
            }
            Some(NodeId::Tab(_) | NodeId::Window(_)) => {
                self.set_error("Select a pane to move".into());
            }
            None => {}
        }
    }

    fn enter_confirm_close(&mut self) {
        match self.tree_state.selected().last() {
            Some(NodeId::Pane(id)) => {
                let id = *id;
                self.mode = Mode::Confirm {
                    action: PendingAction::ClosePanes(vec![id]),
                    label: format!("Close pane {}?", id),
                };
            }
            Some(NodeId::Tab(id)) => {
                let id = *id;
                if let Some(tab) = self.find_tab(id) {
                    let pane_ids: Vec<u64> = tab.panes.iter().map(|p| p.pane_id).collect();
                    let name = tab.title.as_deref().unwrap_or("(unnamed)");
                    let label = format!("Close tab '{}'? ({} pane(s))", name, pane_ids.len());
                    self.mode = Mode::Confirm {
                        action: PendingAction::ClosePanes(pane_ids),
                        label,
                    };
                }
            }
            Some(NodeId::Window(id)) => {
                let id = *id;
                if let Some(w) = self.windows.iter().find(|w| w.window_id == id) {
                    let pane_ids: Vec<u64> = w.tabs.iter()
                        .flat_map(|t| t.panes.iter().map(|p| p.pane_id))
                        .collect();
                    let name = w.title.as_deref().unwrap_or("(unnamed)");
                    let label = format!("Close window '{}'? ({} pane(s))", name, pane_ids.len());
                    self.mode = Mode::Confirm {
                        action: PendingAction::ClosePanes(pane_ids),
                        label,
                    };
                }
            }
            None => {}
        }
    }

    fn enter_search_mode(&mut self, direct_launch: bool, initial_query: String) {
        let entries = search::build_search_entries(&self.windows);
        let results = search::filter(&entries, &initial_query);
        let cursor = initial_query.len();
        self.mode = Mode::Search {
            query: initial_query,
            cursor,
            entries,
            results,
            selected_index: 0,
            direct_launch,
        };
    }

    fn handle_key_search(&mut self, key: KeyEvent) {
        let is_ctrl_n = key.code == KeyCode::Char('n') && key.modifiers.contains(KeyModifiers::CONTROL);
        let is_ctrl_p = key.code == KeyCode::Char('p') && key.modifiers.contains(KeyModifiers::CONTROL);

        if matches!(key.code, KeyCode::Down) || matches!(key.code, KeyCode::Char('j')) || is_ctrl_n {
            if let Mode::Search { results, selected_index, .. } = &mut self.mode {
                if *selected_index + 1 < results.len() {
                    *selected_index += 1;
                }
            }
        } else if matches!(key.code, KeyCode::Up) || matches!(key.code, KeyCode::Char('k')) || is_ctrl_p {
            if let Mode::Search { selected_index, .. } = &mut self.mode {
                *selected_index = selected_index.saturating_sub(1);
            }
        } else {
            match key.code {
                KeyCode::Enter => self.execute_search_selection(),
                KeyCode::Esc => {
                    let direct = matches!(self.mode, Mode::Search { direct_launch: true, .. });
                    if direct {
                        self.should_quit = true;
                    } else {
                        self.mode = Mode::Normal;
                    }
                }
                KeyCode::Backspace => {
                    if let Mode::Search { query, cursor, .. } = &mut self.mode {
                        if *cursor > 0 {
                            query.remove(*cursor - 1);
                            *cursor -= 1;
                        }
                    }
                    self.refilter_search();
                }
                KeyCode::Delete => {
                    if let Mode::Search { query, cursor, .. } = &mut self.mode {
                        if *cursor < query.len() {
                            query.remove(*cursor);
                        }
                    }
                    self.refilter_search();
                }
                KeyCode::Left => {
                    if let Mode::Search { cursor, .. } = &mut self.mode {
                        *cursor = cursor.saturating_sub(1);
                    }
                }
                KeyCode::Right => {
                    if let Mode::Search { query, cursor, .. } = &mut self.mode {
                        if *cursor < query.len() {
                            *cursor += 1;
                        }
                    }
                }
                KeyCode::Home => {
                    if let Mode::Search { cursor, .. } = &mut self.mode {
                        *cursor = 0;
                    }
                }
                KeyCode::End => {
                    if let Mode::Search { query, cursor, .. } = &mut self.mode {
                        *cursor = query.len();
                    }
                }
                KeyCode::Char(c) => {
                    if let Mode::Search { query, cursor, .. } = &mut self.mode {
                        query.insert(*cursor, c);
                        *cursor += 1;
                    }
                    self.refilter_search();
                }
                _ => {}
            }
        }
    }

    fn execute_search_selection(&mut self) {
        let pane_id = if let Mode::Search { ref entries, ref results, selected_index, .. } = self.mode {
            results.get(selected_index).map(|r| entries[r.entry_index].pane_id)
        } else {
            None
        };

        if let Some(pane_id) = pane_id {
            match wezterm::activate_pane(pane_id) {
                Ok(()) => self.should_quit = true,
                Err(e) => {
                    self.mode = Mode::Normal;
                    self.set_error(format!("Focus failed: {e}"));
                }
            }
        }
    }

    fn refilter_search(&mut self) {
        if let Mode::Search { ref query, ref entries, ref mut results, ref mut selected_index, .. } = self.mode {
            *results = search::filter(entries, query);
            if *selected_index >= results.len() {
                *selected_index = results.len().saturating_sub(1);
            }
        }
    }

    pub fn new_find_mode(current_pane_id: Option<u64>, initial_query: Option<String>) -> Result<Self> {
        let mut app = Self::new(current_pane_id)?;
        app.enter_search_mode(true, initial_query.unwrap_or_default());
        Ok(app)
    }

    // -- Action execution --

    fn execute_rename(&mut self) {
        let input = if let Mode::Rename { ref input, .. } = self.mode {
            input.clone()
        } else {
            return;
        };

        let selected = self.tree_state.selected().to_vec();
        let result = match selected.last() {
            Some(NodeId::Tab(tab_id)) => {
                let tab_id = *tab_id;
                self.find_tab(tab_id)
                    .and_then(|t| t.panes.first().map(|p| p.pane_id))
                    .map(|pane_id| wezterm::set_tab_title(pane_id, &input))
                    .unwrap_or(Ok(()))
            }
            Some(NodeId::Window(window_id)) => {
                let window_id = *window_id;
                self.windows.iter()
                    .find(|w| w.window_id == window_id)
                    .and_then(|w| w.tabs.first())
                    .and_then(|t| t.panes.first())
                    .map(|p| wezterm::set_window_title(p.pane_id, &input))
                    .unwrap_or(Ok(()))
            }
            _ => Ok(()),
        };

        self.mode = Mode::Normal;
        match result {
            Ok(()) => {
                self.set_success(format!("Renamed to '{}'", input));
                self.refresh_data();
            }
            Err(e) => self.set_error(format!("Rename failed: {e}")),
        }
    }

    fn execute_move(&mut self) {
        let (target_window_id, selected_index) = if let Mode::Move { ref window_choices, selected_index } = self.mode {
            (window_choices[selected_index].0, selected_index)
        } else {
            return;
        };

        let pane_id = match self.tree_state.selected().last() {
            Some(NodeId::Pane(id)) => *id,
            _ => return,
        };

        let _ = selected_index; // used only to extract target
        self.mode = Mode::Normal;
        match wezterm::move_pane_to_window(pane_id, target_window_id) {
            Ok(()) => {
                self.set_success(format!("Moved pane {} to window {}", pane_id, target_window_id));
                self.refresh_data();
            }
            Err(e) => self.set_error(format!("Move failed: {e}")),
        }
    }

    fn execute_close(&mut self) {
        let pane_ids = if let Mode::Confirm { action: PendingAction::ClosePanes(ref ids), .. } = self.mode {
            ids.clone()
        } else {
            return;
        };

        self.mode = Mode::Normal;

        let mut errors = Vec::new();
        for id in &pane_ids {
            if let Err(e) = wezterm::kill_pane(*id) {
                errors.push(format!("pane {}: {}", id, e));
            }
        }

        if errors.is_empty() {
            self.set_success(format!("Closed {} pane(s)", pane_ids.len()));
        } else {
            self.set_error(format!("Close errors: {}", errors.join(", ")));
        }

        self.refresh_data();
    }

    // -- Helpers --

    pub fn find_tab(&self, tab_id: u64) -> Option<&WezTab> {
        self.windows.iter()
            .flat_map(|w| w.tabs.iter())
            .find(|t| t.tab_id == tab_id)
    }

    pub fn find_pane_tab(&self, pane_id: u64) -> Option<&WezTab> {
        self.windows
            .iter()
            .flat_map(|w| w.tabs.iter())
            .find(|t| t.panes.iter().any(|p| p.pane_id == pane_id))
    }

    fn find_pane_window(&self, pane_id: u64) -> Option<u64> {
        for w in &self.windows {
            for t in &w.tabs {
                if t.panes.iter().any(|p| p.pane_id == pane_id) {
                    return Some(w.window_id);
                }
            }
        }
        None
    }

    fn refresh_data(&mut self) {
        match wezterm::list_panes() {
            Ok(panes) => {
                self.windows = model::build_tree(&panes);
                if !self.selection_still_valid() {
                    self.tree_state.select_first();
                }
            }
            Err(e) => {
                self.set_error(format!("Refresh failed: {e}"));
            }
        }
    }

    fn selection_still_valid(&self) -> bool {
        match self.tree_state.selected().last() {
            Some(NodeId::Window(id)) => self.windows.iter().any(|w| w.window_id == *id),
            Some(NodeId::Tab(id)) => self.windows.iter()
                .flat_map(|w| &w.tabs)
                .any(|t| t.tab_id == *id),
            Some(NodeId::Pane(id)) => self.windows.iter()
                .flat_map(|w| &w.tabs)
                .flat_map(|t| &t.panes)
                .any(|p| p.pane_id == *id),
            None => true,
        }
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

    fn set_error(&mut self, text: String) {
        self.status_message = Some(StatusMessage { text, is_error: true });
    }

    fn set_success(&mut self, text: String) {
        self.status_message = Some(StatusMessage { text, is_error: false });
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
                    let is_active_tab = tab.panes.iter().any(|p| p.is_active);
                    let active_marker = if is_active_tab { "● " } else { "" };
                    let tab_label = format!(
                        "{}{}  ({} pane{})",
                        active_marker,
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

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

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

    fn app_with_selection(windows: Vec<WezWindow>, selection: Vec<NodeId>) -> App {
        let mut tree_state = TreeState::default();
        tree_state.select(selection);
        App {
            should_quit: false,
            windows,
            tree_state,
            current_pane_id: None,
            mode: Mode::Normal,
            status_message: None,
        }
    }

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
            mode: Mode::Normal,
            status_message: None,
        }
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

        let items = build_tree_items(&windows, Some(100));
        let pane_text = format!("{:?}", items);
        assert!(pane_text.contains("*"), "expected current pane marker in tree items");

        let items = build_tree_items(&windows, None);
        let pane_text = format!("{:?}", items);
        assert!(!pane_text.contains("*"), "unexpected marker when no current pane");
    }

    // -- selected_info tests --

    #[test]
    fn selected_info_for_window() {
        let app = app_with_selection(sample_windows(), vec![NodeId::Window(1)]);
        let info = app.selected_info();
        assert!(info.contains("Window 1"));
        assert!(info.contains("Dev"));
        assert!(info.contains("2 tab(s)"));
    }

    #[test]
    fn selected_info_for_tab() {
        let app = app_with_selection(sample_windows(), vec![NodeId::Window(1), NodeId::Tab(10)]);
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

    // -- Normal mode key handling --

    #[test]
    fn quit_on_q() {
        let mut app = app_with_windows(sample_windows());
        assert!(!app.should_quit);
        app.handle_key(key(KeyCode::Char('q')));
        assert!(app.should_quit);
    }

    #[test]
    fn quit_on_esc() {
        let mut app = app_with_windows(sample_windows());
        app.handle_key(key(KeyCode::Esc));
        assert!(app.should_quit);
    }

    #[test]
    fn navigation_j_k_moves_selection() {
        let mut app = app_with_windows(sample_windows());

        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal.draw(|frame| crate::ui::draw(frame, &mut app)).unwrap();

        app.handle_key(key(KeyCode::Char('j')));
        terminal.draw(|frame| crate::ui::draw(frame, &mut app)).unwrap();
        app.handle_key(key(KeyCode::Char('j')));
        terminal.draw(|frame| crate::ui::draw(frame, &mut app)).unwrap();
        let pos_after_two_j = app.tree_state.selected().to_vec();

        app.handle_key(key(KeyCode::Char('k')));
        terminal.draw(|frame| crate::ui::draw(frame, &mut app)).unwrap();
        let pos_after_k = app.tree_state.selected().to_vec();

        assert_ne!(pos_after_two_j, pos_after_k, "k should move selection up");
    }

    #[test]
    fn unknown_key_is_ignored() {
        let mut app = app_with_windows(sample_windows());
        let before = app.tree_state.selected().to_vec();
        app.handle_key(key(KeyCode::Char('z')));
        let after = app.tree_state.selected().to_vec();
        assert_eq!(before, after);
        assert!(!app.should_quit);
    }

    // -- Enter/focus behavior --

    #[test]
    fn enter_on_window_toggles_tree() {
        let mut app = app_with_selection(sample_windows(), vec![NodeId::Window(2)]);
        // Window 2 is closed by default; Enter should toggle (not quit)
        app.handle_key(key(KeyCode::Enter));
        assert!(!app.should_quit);
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn enter_on_tab_toggles_tree() {
        let mut app = app_with_selection(
            sample_windows(),
            vec![NodeId::Window(1), NodeId::Tab(10)],
        );
        app.handle_key(key(KeyCode::Enter));
        assert!(!app.should_quit);
        assert_eq!(app.mode, Mode::Normal);
    }

    // -- Rename mode --

    #[test]
    fn enter_rename_on_tab() {
        let mut app = app_with_selection(
            sample_windows(),
            vec![NodeId::Window(1), NodeId::Tab(10)],
        );
        app.handle_key(key(KeyCode::Char('r')));
        assert_eq!(
            app.mode,
            Mode::Rename { input: "editor".to_string(), cursor: 6 },
        );
    }

    #[test]
    fn enter_rename_on_window() {
        let mut app = app_with_selection(sample_windows(), vec![NodeId::Window(1)]);
        app.handle_key(key(KeyCode::Char('r')));
        assert_eq!(
            app.mode,
            Mode::Rename { input: "Dev".to_string(), cursor: 3 },
        );
    }

    #[test]
    fn enter_rename_on_pane_shows_error() {
        let mut app = app_with_selection(
            sample_windows(),
            vec![NodeId::Window(1), NodeId::Tab(10), NodeId::Pane(100)],
        );
        app.handle_key(key(KeyCode::Char('r')));
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.status_message.as_ref().unwrap().is_error);
        assert!(app.status_message.as_ref().unwrap().text.contains("Cannot rename"));
    }

    #[test]
    fn cancel_rename_with_esc() {
        let mut app = app_with_selection(
            sample_windows(),
            vec![NodeId::Window(1), NodeId::Tab(10)],
        );
        app.handle_key(key(KeyCode::Char('r')));
        assert!(matches!(app.mode, Mode::Rename { .. }));
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Normal);
        assert!(!app.should_quit); // Esc in rename cancels, doesn't quit
    }

    #[test]
    fn rename_input_typing() {
        let mut app = app_with_selection(sample_windows(), vec![NodeId::Window(1)]);
        app.mode = Mode::Rename { input: String::new(), cursor: 0 };
        app.handle_key(key(KeyCode::Char('h')));
        app.handle_key(key(KeyCode::Char('i')));
        assert_eq!(
            app.mode,
            Mode::Rename { input: "hi".to_string(), cursor: 2 },
        );
    }

    #[test]
    fn rename_input_backspace() {
        let mut app = app_with_selection(sample_windows(), vec![NodeId::Window(1)]);
        app.mode = Mode::Rename { input: "hello".to_string(), cursor: 5 };
        app.handle_key(key(KeyCode::Backspace));
        assert_eq!(
            app.mode,
            Mode::Rename { input: "hell".to_string(), cursor: 4 },
        );
    }

    #[test]
    fn rename_input_cursor_movement() {
        let mut app = app_with_selection(sample_windows(), vec![NodeId::Window(1)]);
        app.mode = Mode::Rename { input: "hello".to_string(), cursor: 5 };
        app.handle_key(key(KeyCode::Left));
        app.handle_key(key(KeyCode::Left));
        app.handle_key(key(KeyCode::Char('X')));
        assert_eq!(
            app.mode,
            Mode::Rename { input: "helXlo".to_string(), cursor: 4 },
        );
    }

    #[test]
    fn q_in_rename_types_q() {
        let mut app = app_with_selection(sample_windows(), vec![NodeId::Window(1)]);
        app.mode = Mode::Rename { input: String::new(), cursor: 0 };
        app.handle_key(key(KeyCode::Char('q')));
        assert!(!app.should_quit);
        assert_eq!(
            app.mode,
            Mode::Rename { input: "q".to_string(), cursor: 1 },
        );
    }

    // -- Move mode --

    #[test]
    fn enter_move_on_pane() {
        let mut app = app_with_selection(
            sample_windows(),
            vec![NodeId::Window(1), NodeId::Tab(10), NodeId::Pane(100)],
        );
        app.handle_key(key(KeyCode::Char('m')));
        if let Mode::Move { ref window_choices, selected_index } = app.mode {
            // Should offer window 2 (not window 1, which contains the pane)
            assert_eq!(window_choices.len(), 1);
            assert_eq!(window_choices[0].0, 2);
            assert_eq!(selected_index, 0);
        } else {
            panic!("expected Move mode, got {:?}", app.mode);
        }
    }

    #[test]
    fn enter_move_on_tab_shows_error() {
        let mut app = app_with_selection(
            sample_windows(),
            vec![NodeId::Window(1), NodeId::Tab(10)],
        );
        app.handle_key(key(KeyCode::Char('m')));
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.status_message.as_ref().unwrap().is_error);
        assert!(app.status_message.as_ref().unwrap().text.contains("Select a pane"));
    }

    #[test]
    fn enter_move_single_window_shows_error() {
        let windows = vec![
            make_window(1, "Only", vec![
                make_tab(10, "tab", vec![make_pane(100, "zsh", "/tmp")]),
            ]),
        ];
        let mut app = app_with_selection(
            windows,
            vec![NodeId::Window(1), NodeId::Tab(10), NodeId::Pane(100)],
        );
        app.handle_key(key(KeyCode::Char('m')));
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.status_message.as_ref().unwrap().text.contains("No other windows"));
    }

    #[test]
    fn cancel_move_with_esc() {
        let mut app = app_with_selection(
            sample_windows(),
            vec![NodeId::Window(1), NodeId::Tab(10), NodeId::Pane(100)],
        );
        app.handle_key(key(KeyCode::Char('m')));
        assert!(matches!(app.mode, Mode::Move { .. }));
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn move_mode_j_k_navigation() {
        let windows = vec![
            make_window(1, "A", vec![
                make_tab(10, "t", vec![make_pane(100, "zsh", "/tmp")]),
            ]),
            make_window(2, "B", vec![
                make_tab(20, "t", vec![make_pane(200, "zsh", "/tmp")]),
            ]),
            make_window(3, "C", vec![
                make_tab(30, "t", vec![make_pane(300, "zsh", "/tmp")]),
            ]),
        ];
        let mut app = app_with_selection(
            windows,
            vec![NodeId::Window(1), NodeId::Tab(10), NodeId::Pane(100)],
        );
        app.handle_key(key(KeyCode::Char('m')));
        // Should have 2 choices (windows 2 and 3)
        assert!(matches!(app.mode, Mode::Move { ref window_choices, .. } if window_choices.len() == 2));

        app.handle_key(key(KeyCode::Char('j')));
        if let Mode::Move { selected_index, .. } = app.mode {
            assert_eq!(selected_index, 1);
        }

        app.handle_key(key(KeyCode::Char('k')));
        if let Mode::Move { selected_index, .. } = app.mode {
            assert_eq!(selected_index, 0);
        }
    }

    #[test]
    fn move_mode_j_stays_in_bounds() {
        let mut app = app_with_selection(
            sample_windows(),
            vec![NodeId::Window(1), NodeId::Tab(10), NodeId::Pane(100)],
        );
        app.handle_key(key(KeyCode::Char('m')));
        // Only 1 choice (window 2), pressing j shouldn't go past it
        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(key(KeyCode::Char('j')));
        if let Mode::Move { selected_index, window_choices } = &app.mode {
            assert_eq!(*selected_index, window_choices.len() - 1);
        }
    }

    // -- Confirm/close mode --

    #[test]
    fn enter_confirm_close_pane() {
        let mut app = app_with_selection(
            sample_windows(),
            vec![NodeId::Window(1), NodeId::Tab(10), NodeId::Pane(100)],
        );
        app.handle_key(key(KeyCode::Char('x')));
        assert_eq!(
            app.mode,
            Mode::Confirm {
                action: PendingAction::ClosePanes(vec![100]),
                label: "Close pane 100?".to_string(),
            },
        );
    }

    #[test]
    fn enter_confirm_close_tab() {
        let mut app = app_with_selection(
            sample_windows(),
            vec![NodeId::Window(1), NodeId::Tab(10)],
        );
        app.handle_key(key(KeyCode::Char('x')));
        if let Mode::Confirm { action: PendingAction::ClosePanes(ids), ref label } = app.mode {
            assert_eq!(ids, vec![100, 101]); // both panes in tab 10
            assert!(label.contains("editor"));
            assert!(label.contains("2 pane(s)"));
        } else {
            panic!("expected Confirm mode");
        }
    }

    #[test]
    fn enter_confirm_close_window() {
        let mut app = app_with_selection(sample_windows(), vec![NodeId::Window(1)]);
        app.handle_key(key(KeyCode::Char('x')));
        if let Mode::Confirm { action: PendingAction::ClosePanes(ids), ref label } = app.mode {
            assert_eq!(ids, vec![100, 101, 102]); // all panes in window 1
            assert!(label.contains("Dev"));
            assert!(label.contains("3 pane(s)"));
        } else {
            panic!("expected Confirm mode");
        }
    }

    #[test]
    fn cancel_confirm_with_n() {
        let mut app = app_with_selection(
            sample_windows(),
            vec![NodeId::Window(1), NodeId::Tab(10), NodeId::Pane(100)],
        );
        app.handle_key(key(KeyCode::Char('x')));
        assert!(matches!(app.mode, Mode::Confirm { .. }));
        app.handle_key(key(KeyCode::Char('n')));
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn cancel_confirm_with_esc() {
        let mut app = app_with_selection(
            sample_windows(),
            vec![NodeId::Window(1), NodeId::Tab(10), NodeId::Pane(100)],
        );
        app.handle_key(key(KeyCode::Char('x')));
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Normal);
    }

    // -- Status message --

    #[test]
    fn status_message_clears_on_normal_keypress() {
        let mut app = app_with_windows(sample_windows());
        app.status_message = Some(StatusMessage {
            text: "test".into(),
            is_error: false,
        });
        app.handle_key(key(KeyCode::Char('j')));
        assert!(app.status_message.is_none());
    }

    // -- Helper tests --

    #[test]
    fn find_tab_returns_correct_tab() {
        let app = app_with_windows(sample_windows());
        let tab = app.find_tab(10).unwrap();
        assert_eq!(tab.title.as_deref(), Some("editor"));
        assert!(app.find_tab(999).is_none());
    }

    #[test]
    fn find_pane_window_returns_correct_window() {
        let app = app_with_windows(sample_windows());
        assert_eq!(app.find_pane_window(100), Some(1));
        assert_eq!(app.find_pane_window(200), Some(2));
        assert_eq!(app.find_pane_window(999), None);
    }

    #[test]
    fn selection_still_valid_checks_model() {
        let mut app = app_with_selection(sample_windows(), vec![NodeId::Pane(100)]);
        assert!(app.selection_still_valid());

        // Remove all windows — selection should be invalid
        app.windows.clear();
        assert!(!app.selection_still_valid());
    }

    // -- Active tab marker --

    #[test]
    fn active_tab_shows_marker() {
        let mut pane = make_pane(100, "nvim", "/home/user/code");
        pane.is_active = true;
        let windows = vec![
            make_window(1, "Dev", vec![
                make_tab(10, "editor", vec![pane]),
                make_tab(11, "logs", vec![make_pane(101, "tail", "/var/log")]),
            ]),
        ];
        let items = build_tree_items(&windows, None);
        let debug = format!("{:?}", items);
        // Active tab should have ● marker, inactive should not
        assert!(debug.contains("● editor"), "active tab should have ● marker");
        assert!(!debug.contains("● logs"), "inactive tab should not have ● marker");
    }

    #[test]
    fn inactive_tabs_have_no_marker() {
        let windows = sample_windows(); // all panes have is_active = false
        let items = build_tree_items(&windows, None);
        let debug = format!("{:?}", items);
        assert!(!debug.contains("●"), "no tabs should have active marker");
    }

    // -- Search mode --

    #[test]
    fn slash_enters_search_mode() {
        let mut app = app_with_windows(sample_windows());
        app.handle_key(key(KeyCode::Char('/')));
        assert!(matches!(app.mode, Mode::Search { .. }));
        if let Mode::Search { ref query, cursor, ref results, .. } = app.mode {
            assert!(query.is_empty());
            assert_eq!(cursor, 0);
            assert_eq!(results.len(), 4); // all panes shown when query empty
        }
    }

    #[test]
    fn search_mode_typing() {
        let mut app = app_with_windows(sample_windows());
        app.handle_key(key(KeyCode::Char('/')));
        app.handle_key(key(KeyCode::Char('n')));
        app.handle_key(key(KeyCode::Char('v')));
        if let Mode::Search { ref query, cursor, .. } = app.mode {
            assert_eq!(query, "nv");
            assert_eq!(cursor, 2);
        } else {
            panic!("expected Search mode");
        }
    }

    #[test]
    fn search_mode_backspace() {
        let mut app = app_with_windows(sample_windows());
        app.handle_key(key(KeyCode::Char('/')));
        app.handle_key(key(KeyCode::Char('a')));
        app.handle_key(key(KeyCode::Char('b')));
        app.handle_key(key(KeyCode::Backspace));
        if let Mode::Search { ref query, cursor, .. } = app.mode {
            assert_eq!(query, "a");
            assert_eq!(cursor, 1);
        }
    }

    #[test]
    fn search_mode_esc_returns_to_normal() {
        let mut app = app_with_windows(sample_windows());
        app.handle_key(key(KeyCode::Char('/')));
        assert!(matches!(app.mode, Mode::Search { .. }));
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Normal);
        assert!(!app.should_quit);
    }

    #[test]
    fn search_mode_esc_quits_in_direct_launch() {
        let mut app = app_with_windows(sample_windows());
        app.enter_search_mode(true, String::new());
        app.handle_key(key(KeyCode::Esc));
        assert!(app.should_quit);
    }

    #[test]
    fn search_mode_j_k_navigation() {
        let mut app = app_with_windows(sample_windows());
        app.handle_key(key(KeyCode::Char('/')));
        // Empty query shows all 4 panes
        app.handle_key(key(KeyCode::Char('j')));
        if let Mode::Search { selected_index, .. } = app.mode {
            assert_eq!(selected_index, 1);
        }
        app.handle_key(key(KeyCode::Char('k')));
        if let Mode::Search { selected_index, .. } = app.mode {
            assert_eq!(selected_index, 0);
        }
    }

    #[test]
    fn search_mode_selection_resets_on_query_change() {
        let mut app = app_with_windows(sample_windows());
        app.handle_key(key(KeyCode::Char('/')));
        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(key(KeyCode::Char('j')));
        // Now type something — selection should reset to 0
        app.handle_key(key(KeyCode::Char('a')));
        if let Mode::Search { selected_index, .. } = app.mode {
            assert_eq!(selected_index, 0);
        }
    }

    #[test]
    fn search_mode_j_stays_in_bounds() {
        let mut app = app_with_windows(sample_windows());
        app.handle_key(key(KeyCode::Char('/')));
        for _ in 0..20 {
            app.handle_key(key(KeyCode::Char('j')));
        }
        if let Mode::Search { selected_index, ref results, .. } = app.mode {
            assert!(selected_index < results.len());
        }
    }

    #[test]
    fn q_in_search_types_q() {
        let mut app = app_with_windows(sample_windows());
        app.handle_key(key(KeyCode::Char('/')));
        app.handle_key(key(KeyCode::Char('q')));
        assert!(!app.should_quit);
        if let Mode::Search { ref query, .. } = app.mode {
            assert_eq!(query, "q");
        }
    }
}
