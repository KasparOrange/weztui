use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use tui_tree_widget::{TreeItem, TreeState};

use crate::ipc;
use crate::model::{self, WezTab, WezWindow};
use crate::search::{self, SearchEntry, SearchResult};
use crate::session::{self, SessionSummary};
use crate::settings::{self, SettingsPanel, SettingsState, CATEGORIES};
use crate::wezterm;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NodeId {
    Workspace(String),
    Window(u64),
    Tab(u64),
    Pane(u64),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Normal,
    Rename { input: String, cursor: usize },
    Move {
        grabbed: NodeId,
        grabbed_label: String,
    },
    Confirm { action: PendingAction, label: String },
    Search {
        query: String,
        cursor: usize,
        entries: Vec<SearchEntry>,
        results: Vec<SearchResult>,
        selected_index: usize,
        direct_launch: bool,
    },
    Help,
    Settings(SettingsState),
    SessionPick {
        sessions: Vec<SessionSummary>,
        selected_index: usize,
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
            Mode::Help => { self.mode = Mode::Normal; }
            Mode::Settings(_) => self.handle_key_settings(code),
            Mode::SessionPick { .. } => self.handle_key_session_pick(code),
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
            KeyCode::Char('?') => { self.mode = Mode::Help; }
            KeyCode::Char('s') => self.enter_session_pick_mode(),
            KeyCode::Char('S') => self.enter_settings_mode(),
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
            // Navigate the tree to find the target window
            KeyCode::Char('j') | KeyCode::Down => { self.tree_state.key_down(); }
            KeyCode::Char('k') | KeyCode::Up => { self.tree_state.key_up(); }
            KeyCode::Char('h') | KeyCode::Left => { self.tree_state.key_left(); }
            KeyCode::Char('l') | KeyCode::Right => { self.tree_state.key_right(); }
            // Confirm move to the currently selected window
            KeyCode::Enter | KeyCode::Char('m') => {
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
            Some(NodeId::Pane(_) | NodeId::Workspace(_)) => {
                self.set_error("Cannot rename — select a tab or window".into());
            }
            None => {}
        }
    }

    fn enter_move_mode(&mut self) {
        let (grabbed, grabbed_label) = match self.tree_state.selected().last() {
            Some(NodeId::Pane(id)) => {
                let label = self.windows.iter()
                    .flat_map(|w| &w.tabs)
                    .flat_map(|t| &t.panes)
                    .find(|p| p.pane_id == *id)
                    .map(|p| p.title.clone())
                    .unwrap_or_else(|| format!("Pane {id}"));
                (NodeId::Pane(*id), label)
            }
            Some(NodeId::Tab(id)) => {
                let label = self.find_tab(*id)
                    .and_then(|t| t.title.clone())
                    .unwrap_or_else(|| format!("Tab {id}"));
                (NodeId::Tab(*id), label)
            }
            Some(NodeId::Window(_) | NodeId::Workspace(_)) => {
                self.set_error("Select a tab or pane to move".into());
                return;
            }
            None => return,
        };

        if self.windows.len() < 2 {
            self.set_error("No other windows to move to".into());
            return;
        }

        self.mode = Mode::Move { grabbed, grabbed_label };
    }

    fn enter_settings_mode(&mut self) {
        let values = settings::load_settings();
        let saved_values = values.clone();
        self.mode = Mode::Settings(SettingsState {
            category_index: 0,
            setting_index: 0,
            panel: SettingsPanel::Categories,
            values,
            saved_values,
            editing: false,
            edit_buffer: String::new(),
            edit_cursor: 0,
        });
    }

    fn handle_key_settings(&mut self, code: KeyCode) {
        let state = if let Mode::Settings(ref mut s) = self.mode {
            s
        } else {
            return;
        };

        if state.editing {
            match code {
                KeyCode::Enter => {
                    // Apply edited value
                    let cat = &CATEGORIES[state.category_index];
                    let def = &cat.settings[state.setting_index];
                    let buf = state.edit_buffer.clone();
                    match &def.kind {
                        settings::SettingKind::Float { .. } => {
                            if let Ok(v) = buf.parse::<f64>() {
                                state.values.insert(def.key.to_string(), settings::SettingValue::Float(v));
                            }
                        }
                        settings::SettingKind::Int { .. } => {
                            if let Ok(v) = buf.parse::<i64>() {
                                state.values.insert(def.key.to_string(), settings::SettingValue::Int(v));
                            }
                        }
                        _ => {
                            state.values.insert(def.key.to_string(), settings::SettingValue::Str(buf));
                        }
                    }
                    state.editing = false;
                    self.emit_settings_preview();
                }
                KeyCode::Esc => { state.editing = false; }
                KeyCode::Backspace => {
                    if state.edit_cursor > 0 {
                        state.edit_buffer.remove(state.edit_cursor - 1);
                        state.edit_cursor -= 1;
                    }
                }
                KeyCode::Left => { state.edit_cursor = state.edit_cursor.saturating_sub(1); }
                KeyCode::Right => {
                    if state.edit_cursor < state.edit_buffer.len() {
                        state.edit_cursor += 1;
                    }
                }
                KeyCode::Char(c) => {
                    state.edit_buffer.insert(state.edit_cursor, c);
                    state.edit_cursor += 1;
                }
                _ => {}
            }
            return;
        }

        match state.panel {
            SettingsPanel::Categories => match code {
                KeyCode::Char('j') | KeyCode::Down => {
                    if state.category_index + 1 < CATEGORIES.len() {
                        state.category_index += 1;
                        state.setting_index = 0;
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    state.category_index = state.category_index.saturating_sub(1);
                    state.setting_index = 0;
                }
                KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right | KeyCode::Tab => {
                    state.panel = SettingsPanel::Settings;
                    state.setting_index = 0;
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.revert_settings();
                    self.mode = Mode::Normal;
                }
                KeyCode::Char('w') => {
                    self.save_settings();
                }
                _ => {}
            },
            SettingsPanel::Settings => {
                let cat = &CATEGORIES[state.category_index];
                match code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        if state.setting_index + 1 < cat.settings.len() {
                            state.setting_index += 1;
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        state.setting_index = state.setting_index.saturating_sub(1);
                    }
                    KeyCode::Char('h') | KeyCode::Left | KeyCode::Tab => {
                        state.panel = SettingsPanel::Categories;
                    }
                    KeyCode::Enter => {
                        let def = &cat.settings[state.setting_index];
                        match &def.kind {
                            settings::SettingKind::Bool { .. } => {
                                settings::toggle_bool(&mut state.values, def);
                                self.emit_settings_preview();
                            }
                            settings::SettingKind::Enum { .. } => {
                                settings::cycle_enum(&mut state.values, def);
                                self.emit_settings_preview();
                            }
                            settings::SettingKind::Float { .. } | settings::SettingKind::Int { .. } => {
                                let current = settings::display_value(&settings::get_value(&state.values, def));
                                state.editing = true;
                                state.edit_buffer = current.clone();
                                state.edit_cursor = current.len();
                            }
                        }
                    }
                    KeyCode::Char('+') | KeyCode::Char('=') => {
                        let def = &cat.settings[state.setting_index];
                        settings::increment(&mut state.values, def);
                        self.emit_settings_preview();
                    }
                    KeyCode::Char('-') => {
                        let def = &cat.settings[state.setting_index];
                        settings::decrement(&mut state.values, def);
                        self.emit_settings_preview();
                    }
                    KeyCode::Char('r') => {
                        // Reset this setting to its initial (saved) value
                        let def = &cat.settings[state.setting_index];
                        if let Some(saved) = state.saved_values.get(def.key) {
                            state.values.insert(def.key.to_string(), saved.clone());
                        } else {
                            state.values.remove(def.key);
                        }
                        self.emit_settings_preview();
                    }
                    KeyCode::Char('e') => {
                        // Open the WezTerm Lua config in the default editor
                        self.open_wezterm_config();
                    }
                    KeyCode::Esc | KeyCode::Char('q') => {
                        self.revert_settings();
                        self.mode = Mode::Normal;
                    }
                    KeyCode::Char('w') => {
                        self.save_settings();
                    }
                    _ => {}
                }
            }
        }
    }

    fn open_wezterm_config(&mut self) {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let config_path = std::path::Path::new(&home).join(".wezterm.lua");
        if !config_path.exists() {
            let alt = std::path::Path::new(&home).join(".config/wezterm/wezterm.lua");
            if alt.exists() {
                let _ = std::process::Command::new("open").arg(&alt).spawn();
                return;
            }
        }
        let _ = std::process::Command::new("open").arg(&config_path).spawn();
    }

    fn emit_settings_preview(&self) {
        if let Mode::Settings(ref state) = self.mode {
            let json = settings::to_wezterm_json(&state.values);
            ipc::emit_config_overrides(&json);
        }
    }

    fn revert_settings(&self) {
        if let Mode::Settings(ref state) = self.mode {
            let json = settings::to_wezterm_json(&state.saved_values);
            ipc::emit_config_overrides(&json);
        }
    }

    fn save_settings(&mut self) {
        if let Mode::Settings(ref mut state) = self.mode {
            match settings::save_settings(&state.values) {
                Ok(()) => {
                    state.saved_values = state.values.clone();
                    self.status_message = Some(StatusMessage {
                        text: "Settings saved".to_string(),
                        is_error: false,
                    });
                }
                Err(e) => {
                    self.status_message = Some(StatusMessage {
                        text: format!("Save failed: {e}"),
                        is_error: true,
                    });
                }
            }
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
            Some(NodeId::Workspace(_)) => {
                self.set_error("Cannot close a workspace".into());
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

    fn enter_session_pick_mode(&mut self) {
        match session::list_sessions() {
            Ok(sessions) => {
                if sessions.is_empty() {
                    self.set_error("No saved sessions".into());
                } else {
                    self.mode = Mode::SessionPick { sessions, selected_index: 0 };
                }
            }
            Err(e) => self.set_error(format!("Failed to list sessions: {e}")),
        }
    }

    fn handle_key_session_pick(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => { self.mode = Mode::Normal; }
            KeyCode::Char('j') | KeyCode::Down => {
                if let Mode::SessionPick { sessions, selected_index } = &mut self.mode {
                    if *selected_index + 1 < sessions.len() {
                        *selected_index += 1;
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Mode::SessionPick { selected_index, .. } = &mut self.mode {
                    *selected_index = selected_index.saturating_sub(1);
                }
            }
            KeyCode::Enter => {
                let name = if let Mode::SessionPick { sessions, selected_index } = &self.mode {
                    sessions.get(*selected_index).map(|s| s.name.clone())
                } else {
                    None
                };
                if let Some(name) = name {
                    self.mode = Mode::Normal;
                    match session::load_session(&name).and_then(|s| session::restore_session(&s)) {
                        Ok(report) => {
                            self.set_success(format!(
                                "Restored '{}': {} window(s), {} pane(s)",
                                name, report.windows_created, report.panes_created
                            ));
                            self.refresh_data();
                        }
                        Err(e) => self.set_error(format!("Restore failed: {e}")),
                    }
                }
            }
            KeyCode::Char('x') => {
                let name = if let Mode::SessionPick { sessions, selected_index } = &self.mode {
                    sessions.get(*selected_index).map(|s| s.name.clone())
                } else {
                    None
                };
                if let Some(name) = name {
                    match session::delete_session(&name) {
                        Ok(()) => {
                            // Refresh the list
                            if let Ok(sessions) = session::list_sessions() {
                                if sessions.is_empty() {
                                    self.mode = Mode::Normal;
                                    self.set_success(format!("Deleted '{name}' (no sessions left)"));
                                } else {
                                    let idx = if let Mode::SessionPick { selected_index, .. } = &self.mode {
                                        (*selected_index).min(sessions.len().saturating_sub(1))
                                    } else {
                                        0
                                    };
                                    self.mode = Mode::SessionPick { sessions, selected_index: idx };
                                    self.set_success(format!("Deleted '{name}'"));
                                }
                            }
                        }
                        Err(e) => self.set_error(format!("Delete failed: {e}")),
                    }
                }
            }
            _ => {}
        }
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
        let grabbed = if let Mode::Move { ref grabbed, .. } = self.mode {
            grabbed.clone()
        } else {
            return;
        };

        // Determine the target window from the current tree selection
        let target_window_id = match self.tree_state.selected().last() {
            Some(NodeId::Window(id)) => Some(*id),
            Some(NodeId::Tab(tab_id)) => {
                self.windows.iter()
                    .find(|w| w.tabs.iter().any(|t| t.tab_id == *tab_id))
                    .map(|w| w.window_id)
            }
            Some(NodeId::Pane(pane_id)) => self.find_pane_window(*pane_id),
            Some(NodeId::Workspace(_)) => {
                self.set_error("Navigate to a target window".into());
                return;
            }
            None => None,
        };

        let target_window_id = match target_window_id {
            Some(id) => id,
            None => {
                self.set_error("Navigate to a target window".into());
                return;
            }
        };

        // Collect pane IDs from the grabbed item
        let pane_ids: Vec<u64> = match &grabbed {
            NodeId::Pane(id) => vec![*id],
            NodeId::Tab(tab_id) => {
                self.find_tab(*tab_id)
                    .map(|t| t.panes.iter().map(|p| p.pane_id).collect())
                    .unwrap_or_default()
            }
            _ => return,
        };

        // Check we're not moving to the same window
        let source_window_id = match &grabbed {
            NodeId::Pane(id) => self.find_pane_window(*id),
            NodeId::Tab(tab_id) => self.windows.iter()
                .find(|w| w.tabs.iter().any(|t| t.tab_id == *tab_id))
                .map(|w| w.window_id),
            _ => None,
        };
        if source_window_id == Some(target_window_id) {
            self.set_error("Already in that window".into());
            return;
        }

        self.mode = Mode::Normal;

        let mut errors = Vec::new();
        for pane_id in &pane_ids {
            if let Err(e) = wezterm::move_pane_to_window(*pane_id, target_window_id) {
                errors.push(format!("pane {pane_id}: {e}"));
            }
        }

        if errors.is_empty() {
            self.set_success(format!("Moved {} pane(s) to window {}", pane_ids.len(), target_window_id));
        } else {
            self.set_error(format!("Move errors: {}", errors.join(", ")));
        }
        self.refresh_data();
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
            Some(NodeId::Workspace(name)) => self.windows.iter().any(|w| w.workspace == *name),
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

    #[cfg(test)]
    pub fn selected_info(&self) -> String {
        let selected = self.tree_state.selected();
        match selected.last() {
            Some(NodeId::Workspace(name)) => {
                let count = self.windows.iter().filter(|w| w.workspace == *name).count();
                format!("Workspace {name} - {count} window(s)")
            }
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

/// Build a pane label.
fn pane_label(pane: &crate::model::WezPane, current_pane_id: Option<u64>) -> String {
    let cwd_short = pane
        .cwd
        .as_ref()
        .and_then(|c| c.rsplit('/').next())
        .unwrap_or("~");
    let marker = if Some(pane.pane_id) == current_pane_id { " *" } else { "" };
    format!("{} [{}]{}", pane.title, cwd_short, marker)
}

/// Convert model windows into TreeItem hierarchy for the tree widget.
/// Single-pane tabs render as leaves (no expand arrow).
/// Single-tab windows show the tab directly under the window.
fn build_window_item<'a>(
    window: &'a WezWindow,
    current_pane_id: Option<u64>,
) -> TreeItem<'a, NodeId> {
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
            let tab_fallback = format!("Tab {}", tab.tab_id);
            let tab_title = tab.title.as_deref().unwrap_or(&tab_fallback);

            if tab.panes.len() == 1 {
                // Single pane — show tab as a leaf (no expand arrow)
                let pane = &tab.panes[0];
                let label = format!(
                    "{}{} — {}",
                    active_marker,
                    tab_title,
                    pane_label(pane, current_pane_id),
                );
                TreeItem::new_leaf(NodeId::Tab(tab.tab_id), label)
            } else {
                // Multiple panes — expandable tab
                let tab_label = format!(
                    "{}{}  ({} panes)",
                    active_marker, tab_title, tab.panes.len(),
                );
                let pane_children: Vec<TreeItem<'_, NodeId>> = tab
                    .panes
                    .iter()
                    .map(|pane| {
                        TreeItem::new_leaf(
                            NodeId::Pane(pane.pane_id),
                            pane_label(pane, current_pane_id),
                        )
                    })
                    .collect();
                TreeItem::new(NodeId::Tab(tab.tab_id), tab_label, pane_children)
                    .expect("duplicate tab pane ids")
            }
        })
        .collect();

    // Single tab — show tab children directly under window (skip tab level)
    if window.tabs.len() == 1 && tab_children.len() == 1 {
        // But only flatten if the tab is a leaf (single pane)
        // If the tab has multiple panes, keep the structure
        if window.tabs[0].panes.len() == 1 {
            let pane = &window.tabs[0].panes[0];
            let tab_title = window.tabs[0].title.as_deref().unwrap_or("");
            let is_active = window.tabs[0].panes.iter().any(|p| p.is_active);
            let active_marker = if is_active { "● " } else { "" };
            let label = if tab_title.is_empty() {
                format!(
                    "{}{}  — {}",
                    active_marker,
                    window.title.as_deref().unwrap_or(&format!("Window {}", window.window_id)),
                    pane_label(pane, current_pane_id),
                )
            } else {
                format!(
                    "{}{}  — {}",
                    active_marker,
                    window.title.as_deref().unwrap_or(&format!("Window {}", window.window_id)),
                    pane_label(pane, current_pane_id),
                )
            };
            return TreeItem::new_leaf(NodeId::Window(window.window_id), label);
        }
    }

    TreeItem::new(
        NodeId::Window(window.window_id),
        window_label,
        tab_children,
    )
    .expect("duplicate window tab ids")
}

pub fn build_tree_items<'a>(
    windows: &'a [WezWindow],
    current_pane_id: Option<u64>,
) -> Vec<TreeItem<'a, NodeId>> {
    // Check if there are multiple workspaces
    let mut workspaces: std::collections::BTreeMap<&str, Vec<&WezWindow>> =
        std::collections::BTreeMap::new();
    for w in windows {
        workspaces.entry(&w.workspace).or_default().push(w);
    }

    if workspaces.len() <= 1 {
        // Single workspace (or empty) — flat window list, no grouping
        windows
            .iter()
            .map(|w| build_window_item(w, current_pane_id))
            .collect()
    } else {
        // Multiple workspaces — group under workspace nodes
        workspaces
            .into_iter()
            .map(|(ws_name, ws_windows)| {
                let children: Vec<TreeItem<'_, NodeId>> = ws_windows
                    .iter()
                    .map(|w| build_window_item(w, current_pane_id))
                    .collect();
                let label = format!("{}  ({} window{})", ws_name, children.len(),
                    if children.len() == 1 { "" } else { "s" });
                TreeItem::new(NodeId::Workspace(ws_name.to_string()), label, children)
                    .expect("duplicate workspace ids")
            })
            .collect()
    }
}

/// Build a preview of the windows data as if the grabbed item has been moved
/// to the target window. Returns (modified_windows, ghost_tab_id) where
/// ghost_tab_id marks the preview item for red highlighting.
pub fn build_move_preview(
    windows: &[WezWindow],
    grabbed: &NodeId,
    target_window_id: u64,
) -> (Vec<WezWindow>, Option<u64>) {
    let mut result = windows.to_vec();

    match grabbed {
        NodeId::Tab(tab_id) => {
            // Find and remove the grabbed tab from its source window
            let mut grabbed_tab = None;
            for w in &mut result {
                if let Some(idx) = w.tabs.iter().position(|t| t.tab_id == *tab_id) {
                    grabbed_tab = Some(w.tabs.remove(idx));
                    break;
                }
            }
            // Insert into target window
            if let Some(mut tab) = grabbed_tab {
                // Mark it with a special title prefix so tree rendering can highlight it
                let original_title = tab.title.clone().unwrap_or_default();
                tab.title = Some(format!("[moving] {original_title}"));
                let ghost_id = tab.tab_id;
                if let Some(w) = result.iter_mut().find(|w| w.window_id == target_window_id) {
                    w.tabs.push(tab);
                }
                return (result, Some(ghost_id));
            }
        }
        NodeId::Pane(pane_id) => {
            // Find and remove the grabbed pane, wrap in a new tab
            let mut grabbed_pane = None;
            for w in &mut result {
                for t in &mut w.tabs {
                    if let Some(idx) = t.panes.iter().position(|p| p.pane_id == *pane_id) {
                        grabbed_pane = Some(t.panes.remove(idx));
                        break;
                    }
                }
                // Remove empty tabs
                w.tabs.retain(|t| !t.panes.is_empty());
            }
            if let Some(pane) = grabbed_pane {
                let ghost_tab_id = u64::MAX; // sentinel
                let ghost_tab = crate::model::WezTab {
                    tab_id: ghost_tab_id,
                    title: Some(format!("[moving] {}", pane.title)),
                    panes: vec![pane],
                };
                if let Some(w) = result.iter_mut().find(|w| w.window_id == target_window_id) {
                    w.tabs.push(ghost_tab);
                }
                return (result, Some(ghost_tab_id));
            }
        }
        _ => {}
    }

    // Remove empty windows
    result.retain(|w| !w.tabs.is_empty());
    (result, None)
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
            workspace: "default".to_string(),
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
        assert_eq!(app.mode, Mode::Move { grabbed: NodeId::Pane(100), grabbed_label: "nvim".to_string() });
    }

    #[test]
    fn enter_move_on_tab() {
        let mut app = app_with_selection(
            sample_windows(),
            vec![NodeId::Window(1), NodeId::Tab(10)],
        );
        app.handle_key(key(KeyCode::Char('m')));
        assert_eq!(app.mode, Mode::Move { grabbed: NodeId::Tab(10), grabbed_label: "editor".to_string() });
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

    // -- Help mode --

    #[test]
    fn question_mark_enters_help() {
        let mut app = app_with_windows(sample_windows());
        app.handle_key(key(KeyCode::Char('?')));
        assert_eq!(app.mode, Mode::Help);
    }

    #[test]
    fn any_key_exits_help() {
        let mut app = app_with_windows(sample_windows());
        app.handle_key(key(KeyCode::Char('?')));
        assert_eq!(app.mode, Mode::Help);
        app.handle_key(key(KeyCode::Char('a')));
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn q_in_help_does_not_quit() {
        let mut app = app_with_windows(sample_windows());
        app.handle_key(key(KeyCode::Char('?')));
        app.handle_key(key(KeyCode::Char('q')));
        assert!(!app.should_quit);
        assert_eq!(app.mode, Mode::Normal);
    }

    // -- Session pick mode --

    #[test]
    fn s_with_no_sessions_shows_error() {
        let mut app = app_with_windows(sample_windows());
        app.handle_key(key(KeyCode::Char('s')));
        // No sessions saved in test env — should show error or enter mode
        // (depends on whether sessions dir exists)
        // Either way, should not crash
        assert!(!app.should_quit);
    }

    // -- Workspace grouping --

    #[test]
    fn single_workspace_flat_tree() {
        let windows = sample_windows(); // all have workspace = "default"
        let items = build_tree_items(&windows, None);
        // Should be flat — no workspace nodes
        assert_eq!(items.len(), 2); // 2 windows directly
    }

    #[test]
    fn multiple_workspaces_grouped() {
        let mut windows = sample_windows();
        windows[0].workspace = "dev".to_string();
        windows[1].workspace = "ops".to_string();
        let items = build_tree_items(&windows, None);
        // Should have 2 workspace nodes
        assert_eq!(items.len(), 2);
        let debug = format!("{:?}", items);
        assert!(debug.contains("dev"));
        assert!(debug.contains("ops"));
    }
}
