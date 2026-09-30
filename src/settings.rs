use crossterm::event::KeyCode;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use stack_settings::Settings;

/// The app name: `~/.config/weztui/config.toml`, `~/.local/state/weztui/`.
pub const APP: &str = "weztui";

/// weztui's `config.toml`, loaded and written through `stack-settings`.
pub type Store = Settings<Config>;

/// The WezTerm overrides as the dialog sees them: a JSON object holding only
/// the keys that are set. Serialized as-is, it is what gets pushed to WezTerm.
pub type Overrides = Map<String, Value>;

// -- The settings struct (one struct, one file) --

/// weztui's settings (`~/.config/weztui/config.toml`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Config {
    /// WezTerm options that weztui sets on top of your WezTerm Lua config.
    /// A key that is not set here is left to WezTerm.
    pub wezterm: WeztermOverrides,
}

/// Every field is optional: only keys that are set are pushed to WezTerm.
/// The field names are WezTerm's own config keys.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct WeztermOverrides {
    /// Font size in points.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
    /// Line height as a factor of the font's own height.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_height: Option<f64>,
    /// Cell width as a factor of the font's own width.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cell_width: Option<f64>,
    /// Bold text also uses the bright variant of its color.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold_brightens_ansi_colors: Option<bool>,
    /// Resize the window when the font size changes, keeping rows and columns.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adjust_window_size_when_changing_font_size: Option<bool>,
    /// Name of a WezTerm color scheme.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_scheme: Option<String>,
    /// Window opacity, 0 (invisible) to 1 (solid).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_background_opacity: Option<f64>,
    /// Opacity of text background colors, 0 to 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_background_opacity: Option<f64>,
    /// Title bar and resize border, e.g. "TITLE | RESIZE".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_decorations: Option<String>,
    /// "AlwaysPrompt" or "NeverPrompt" when closing a window.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_close_confirmation: Option<String>,
    /// Use the macOS fullscreen space instead of a borderless window.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_macos_fullscreen_mode: Option<bool>,
    /// Columns of a new window.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_cols: Option<i64>,
    /// Rows of a new window.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_rows: Option<i64>,
    /// Show the tab bar.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_tab_bar: Option<bool>,
    /// Native-looking tab bar instead of the plain retro one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_fancy_tab_bar: Option<bool>,
    /// Hide the tab bar while a window has a single tab.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hide_tab_bar_if_only_one_tab: Option<bool>,
    /// Put the tab bar at the bottom of the window.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab_bar_at_bottom: Option<bool>,
    /// Show each tab's number in the tab bar.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_tab_index_in_tab_bar: Option<bool>,
    /// Cursor shape and blinking, e.g. "SteadyBlock".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_cursor_style: Option<String>,
    /// Cursor blink interval in milliseconds; 0 stops blinking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor_blink_rate: Option<i64>,
    /// Draw the cursor by swapping text and background color.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force_reverse_video_cursor: Option<bool>,
    /// Lines of history kept per pane.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scrollback_lines: Option<i64>,
    /// Show a scrollbar.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_scroll_bar: Option<bool>,
    /// Hide the mouse pointer while typing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hide_mouse_cursor_when_typing: Option<bool>,
    /// Focus the pane under the mouse pointer without a click.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pane_focus_follows_mouse: Option<bool>,
    /// What happens to a pane when its program exits: "Close", "Hold" or "CloseOnCleanExit".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_behavior: Option<String>,
    /// "SystemBeep" or "Disabled".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audible_bell: Option<String>,
    /// Reload the WezTerm config when its files change.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automatically_reload_config: Option<bool>,
    /// Upper limit of frames drawn per second.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_fps: Option<i64>,
}

/// Load `~/.config/weztui/config.toml` and refresh the schema file next to it.
/// A missing file means "nothing set" and is not created by this.
pub fn load() -> stack_settings::Result<Store> {
    let mut store = Store::load(APP)?;
    if store.path().exists() {
        store.write_schema()?;
    }
    Ok(store)
}

/// The set keys of the `[wezterm]` table as a JSON object.
pub fn overrides_of(config: &Config) -> Overrides {
    match serde_json::to_value(&config.wezterm) {
        Ok(Value::Object(map)) => map,
        _ => Overrides::new(),
    }
}

/// The JSON pushed to WezTerm as config overrides (live, via the companion plugin).
pub fn to_wezterm_json(values: &Overrides) -> String {
    Value::Object(values.clone()).to_string()
}

// -- The dialog's row table --

#[derive(Debug, Clone, Copy)]
pub enum RowKind {
    Toggle { default: bool },
    Choice { options: &'static [&'static str], default_index: usize },
    Float { default: f64, min: f64, max: f64, step: f64 },
    Int { default: i64, min: i64, max: i64, step: i64 },
}

/// One row of the settings dialog. `id` is the WezTerm config key and the key
/// in the `[wezterm]` table; everything the dialog shows about a setting is here.
/// `desc` is one sentence for the user; for a choice it may follow
/// `opt = explanation · opt = explanation`.
#[derive(Debug)]
pub struct Row {
    pub id: &'static str,
    pub label: &'static str,
    pub desc: &'static str,
    pub kind: RowKind,
}

// A row is its id: two rows are the same setting when the ids match.
impl PartialEq for Row {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[derive(Debug)]
pub struct Category {
    pub name: &'static str,
    pub rows: &'static [Row],
}

pub static CATEGORIES: &[Category] = &[
    Category {
        name: "Font & Text",
        rows: &[
            Row { id: "font_size", label: "Font Size", desc: "How large text is drawn, in points.", kind: RowKind::Float { default: 12.0, min: 6.0, max: 36.0, step: 0.5 } },
            Row { id: "line_height", label: "Line Height", desc: "Space between lines, as a factor of the font's own height.", kind: RowKind::Float { default: 1.0, min: 0.5, max: 2.0, step: 0.1 } },
            Row { id: "cell_width", label: "Cell Width", desc: "Space between characters, as a factor of the font's own width.", kind: RowKind::Float { default: 1.0, min: 0.5, max: 2.0, step: 0.1 } },
            Row { id: "bold_brightens_ansi_colors", label: "Bold Brightens Colors", desc: "Bold text also switches to the bright variant of its color.", kind: RowKind::Toggle { default: true } },
            Row { id: "adjust_window_size_when_changing_font_size", label: "Resize Window on Font Change", desc: "A font size change resizes the window so rows and columns stay the same.", kind: RowKind::Toggle { default: true } },
        ],
    },
    Category {
        name: "Colors & Themes",
        rows: &[
            Row { id: "color_scheme", label: "Color Scheme", desc: "The colors of the whole terminal; each scheme is previewed as you move through the list.", kind: RowKind::Choice {
                options: &[
                    "Gruvbox Dark (Gogh)", "Gruvbox dark, hard (base16)", "Gruvbox dark, medium (base16)",
                    "Gruvbox dark, soft (base16)", "Gruvbox light, hard (base16)",
                    "Solarized (dark) (terminal.sexy)", "Solarized Dark (Gogh)",
                    "Catppuccin Mocha", "Catppuccin Latte", "Catppuccin Frappe", "Catppuccin Macchiato",
                    "Tokyo Night", "Tokyo Night Storm", "Tokyo Night Moon",
                    "Dracula", "Dracula+", "Nord",
                    "One Dark (Gogh)", "One Half Dark (Gogh)",
                    "Kanagawa (Gogh)", "Kanagawa Dragon (Gogh)",
                    "rose-pine", "rose-pine-moon", "rose-pine-dawn",
                    "Monokai Remastered", "GitHub Dark",
                    "Ayu Dark (Gogh)", "Ayu Mirage (Gogh)",
                    "Everforest Dark (Gogh)", "Nightfox",
                ],
                default_index: 0,
            }},
            Row { id: "window_background_opacity", label: "Window Opacity", desc: "How solid the window is: 1 is opaque, lower lets the desktop show through.", kind: RowKind::Float { default: 1.0, min: 0.0, max: 1.0, step: 0.05 } },
            Row { id: "text_background_opacity", label: "Text Background Opacity", desc: "How solid the background color behind colored text is.", kind: RowKind::Float { default: 1.0, min: 0.0, max: 1.0, step: 0.05 } },
        ],
    },
    Category {
        name: "Window",
        rows: &[
            Row { id: "window_decorations", label: "Window Decorations", desc: "TITLE | RESIZE = title bar and resizable border · RESIZE = border only, no title bar · NONE = neither, the window cannot be resized · INTEGRATED_BUTTONS | RESIZE = window buttons inside the tab bar", kind: RowKind::Choice {
                options: &["TITLE | RESIZE", "RESIZE", "NONE", "INTEGRATED_BUTTONS | RESIZE"],
                default_index: 0,
            }},
            Row { id: "window_close_confirmation", label: "Close Confirmation", desc: "AlwaysPrompt = ask before closing a window with running programs · NeverPrompt = close at once", kind: RowKind::Choice {
                options: &["AlwaysPrompt", "NeverPrompt"],
                default_index: 0,
            }},
            Row { id: "native_macos_fullscreen_mode", label: "macOS Native Fullscreen", desc: "Fullscreen moves the window to its own macOS space instead of covering the screen in place.", kind: RowKind::Toggle { default: false } },
            Row { id: "initial_cols", label: "Initial Columns", desc: "How many columns wide a new window opens.", kind: RowKind::Int { default: 80, min: 20, max: 400, step: 10 } },
            Row { id: "initial_rows", label: "Initial Rows", desc: "How many rows tall a new window opens.", kind: RowKind::Int { default: 24, min: 10, max: 200, step: 5 } },
        ],
    },
    Category {
        name: "Tab Bar",
        rows: &[
            Row { id: "enable_tab_bar", label: "Enable Tab Bar", desc: "Show the tab bar.", kind: RowKind::Toggle { default: true } },
            Row { id: "use_fancy_tab_bar", label: "Fancy Tab Bar", desc: "Draw tabs in the native look instead of the plain text style.", kind: RowKind::Toggle { default: true } },
            Row { id: "hide_tab_bar_if_only_one_tab", label: "Hide If Single Tab", desc: "Hide the tab bar while a window has only one tab.", kind: RowKind::Toggle { default: false } },
            Row { id: "tab_bar_at_bottom", label: "Tab Bar at Bottom", desc: "Put the tab bar at the bottom of the window instead of the top.", kind: RowKind::Toggle { default: false } },
            Row { id: "show_tab_index_in_tab_bar", label: "Show Tab Index", desc: "Show each tab's number in front of its title.", kind: RowKind::Toggle { default: true } },
        ],
    },
    Category {
        name: "Cursor",
        rows: &[
            Row { id: "default_cursor_style", label: "Cursor Style", desc: "The cursor's shape, and whether it blinks.", kind: RowKind::Choice {
                options: &["SteadyBlock", "BlinkingBlock", "SteadyUnderline", "BlinkingUnderline", "SteadyBar", "BlinkingBar"],
                default_index: 0,
            }},
            Row { id: "cursor_blink_rate", label: "Blink Rate (ms)", desc: "How long one blink of the cursor takes, in milliseconds; 0 stops the blinking.", kind: RowKind::Int { default: 800, min: 0, max: 2000, step: 100 } },
            Row { id: "force_reverse_video_cursor", label: "Reverse Video Cursor", desc: "Draw the cursor by swapping text and background color instead of using the scheme's cursor color.", kind: RowKind::Toggle { default: false } },
        ],
    },
    Category {
        name: "Scrollback",
        rows: &[
            Row { id: "scrollback_lines", label: "Scrollback Lines", desc: "How many lines of history each pane keeps.", kind: RowKind::Int { default: 3500, min: 0, max: 100000, step: 500 } },
            Row { id: "enable_scroll_bar", label: "Show Scrollbar", desc: "Show a scrollbar at the right edge of each window.", kind: RowKind::Toggle { default: false } },
            Row { id: "hide_mouse_cursor_when_typing", label: "Hide Mouse When Typing", desc: "Hide the mouse pointer while you type.", kind: RowKind::Toggle { default: true } },
            Row { id: "pane_focus_follows_mouse", label: "Focus Follows Mouse", desc: "Moving the mouse over a pane focuses it, without a click.", kind: RowKind::Toggle { default: false } },
        ],
    },
    Category {
        name: "Behavior",
        rows: &[
            Row { id: "exit_behavior", label: "Exit Behavior", desc: "Close = close the pane when its program ends · Hold = keep it open until you close it · CloseOnCleanExit = keep it open only after an error", kind: RowKind::Choice {
                options: &["Close", "Hold", "CloseOnCleanExit"],
                default_index: 0,
            }},
            Row { id: "audible_bell", label: "Audible Bell", desc: "SystemBeep = play the system sound on a bell · Disabled = stay silent", kind: RowKind::Choice {
                options: &["SystemBeep", "Disabled"],
                default_index: 0,
            }},
            Row { id: "automatically_reload_config", label: "Auto-Reload Config", desc: "Apply changes to the WezTerm config files as soon as they are saved.", kind: RowKind::Toggle { default: true } },
            Row { id: "max_fps", label: "Max FPS", desc: "The most frames WezTerm draws per second.", kind: RowKind::Int { default: 60, min: 10, max: 240, step: 10 } },
        ],
    },
];

impl Row {
    /// The dotted key in `config.toml`.
    pub fn file_key(&self) -> String {
        format!("wezterm.{}", self.id)
    }

    /// What WezTerm uses when the key is not set.
    pub fn default_value(&self) -> Value {
        match self.kind {
            RowKind::Toggle { default } => json!(default),
            RowKind::Float { default, .. } => json!(default),
            RowKind::Int { default, .. } => json!(default),
            RowKind::Choice { options, default_index } => json!(options[default_index]),
        }
    }

    /// The set value, or the default when the key is not set.
    pub fn value(&self, values: &Overrides) -> Value {
        values.get(self.id).cloned().unwrap_or_else(|| self.default_value())
    }

    pub fn display(&self, value: &Value) -> String {
        match (self.kind, value) {
            (_, Value::Bool(b)) => if *b { "ON".to_string() } else { "OFF".to_string() },
            (RowKind::Float { .. }, Value::Number(n)) => format!("{:.2}", n.as_f64().unwrap_or(0.0))
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string(),
            (_, Value::String(s)) => s.clone(),
            (_, other) => other.to_string(),
        }
    }

    /// For a choice whose description follows `opt = explanation · …`:
    /// the explanation of one option.
    pub fn explanation(&self, option: &str) -> Option<&'static str> {
        self.desc
            .split(" · ")
            .find_map(|part| part.strip_prefix(option)?.strip_prefix(" = "))
    }

    /// Where the value sits between min and max, 0.0 to 1.0 (numbers only).
    pub fn fraction(&self, values: &Overrides) -> f64 {
        let value = self.value(values);
        let (v, min, max) = match self.kind {
            RowKind::Float { min, max, .. } => (value.as_f64().unwrap_or(min), min, max),
            RowKind::Int { min, max, .. } => (value.as_i64().unwrap_or(min) as f64, min as f64, max as f64),
            _ => return 0.0,
        };
        if max <= min { 0.0 } else { ((v - min) / (max - min)).clamp(0.0, 1.0) }
    }

    fn toggle(&self, values: &mut Overrides) {
        if let Value::Bool(current) = self.value(values) {
            values.insert(self.id.to_string(), json!(!current));
        }
    }

    /// Move a number one step up (`direction` 1) or down (-1), within min and max.
    fn step(&self, values: &mut Overrides, direction: i64) {
        let current = self.value(values);
        match self.kind {
            RowKind::Float { min, max, step, .. } => {
                if let Some(v) = current.as_f64() {
                    let new = (v + step * direction as f64).clamp(min, max);
                    values.insert(self.id.to_string(), json!((new * 100.0).round() / 100.0));
                }
            }
            RowKind::Int { min, max, step, .. } => {
                if let Some(v) = current.as_i64() {
                    values.insert(self.id.to_string(), json!((v + step * direction).clamp(min, max)));
                }
            }
            _ => {}
        }
    }
}

// -- Dialog state: categories → rows → value --

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Level {
    Categories,
    Rows,
    Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SettingsState {
    pub category_index: usize,
    pub row_index: usize,
    pub level: Level,
    /// The set keys; changes while a value is open are live but not yet saved.
    pub values: Overrides,
    /// Value level only: what the row had before it was opened (`None` = not set).
    pub before: Option<Value>,
    /// Value level of a choice: the highlighted option.
    pub choice_index: usize,
}

/// What the app has to do after a key: the dialog state itself touches
/// neither the file nor WezTerm.
#[derive(Debug, PartialEq)]
pub enum Effect {
    None,
    /// Push the current values to WezTerm (not saved yet).
    Preview,
    /// Save this row's value to the file and push to WezTerm.
    Save(&'static Row),
    Close,
    SaveAndClose(&'static Row),
    OpenWeztermConfig,
}

impl SettingsState {
    pub fn new(values: Overrides) -> Self {
        Self {
            category_index: 0,
            row_index: 0,
            level: Level::Categories,
            values,
            before: None,
            choice_index: 0,
        }
    }

    pub fn row(&self) -> &'static Row {
        &CATEGORIES[self.category_index].rows[self.row_index]
    }

    /// One grammar on every level: `j`/`k` move, Enter/`l`/→ go deeper,
    /// Esc/`h`/← go out, `q` and `,` close.
    pub fn handle_key(&mut self, code: KeyCode) -> Effect {
        match self.level {
            Level::Categories => self.key_categories(code),
            Level::Rows => self.key_rows(code),
            Level::Value => self.key_value(code),
        }
    }

    fn key_categories(&mut self, code: KeyCode) -> Effect {
        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                if self.category_index + 1 < CATEGORIES.len() {
                    self.category_index += 1;
                    self.row_index = 0;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.category_index > 0 {
                    self.category_index -= 1;
                    self.row_index = 0;
                }
            }
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => self.level = Level::Rows,
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char(',') => return Effect::Close,
            _ => {}
        }
        Effect::None
    }

    fn key_rows(&mut self, code: KeyCode) -> Effect {
        let row = self.row();
        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                if self.row_index + 1 < CATEGORIES[self.category_index].rows.len() {
                    self.row_index += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => self.row_index = self.row_index.saturating_sub(1),
            KeyCode::Esc | KeyCode::Char('h') | KeyCode::Left => self.level = Level::Categories,
            KeyCode::Char('q') | KeyCode::Char(',') => return Effect::Close,
            KeyCode::Char('e') => return Effect::OpenWeztermConfig,
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => match row.kind {
                RowKind::Toggle { .. } => {
                    row.toggle(&mut self.values);
                    return Effect::Save(row);
                }
                RowKind::Choice { options, .. } => {
                    let current = row.value(&self.values);
                    self.choice_index = options
                        .iter()
                        .position(|o| current.as_str() == Some(o))
                        .unwrap_or(0);
                    self.open_value(row);
                }
                RowKind::Float { .. } | RowKind::Int { .. } => self.open_value(row),
            },
            _ => {}
        }
        Effect::None
    }

    fn open_value(&mut self, row: &Row) {
        self.before = self.values.get(row.id).cloned();
        self.level = Level::Value;
    }

    fn key_value(&mut self, code: KeyCode) -> Effect {
        let row = self.row();
        match code {
            KeyCode::Char('j') | KeyCode::Down => self.change_value(row, -1),
            KeyCode::Char('k') | KeyCode::Up => self.change_value(row, 1),
            KeyCode::Enter | KeyCode::Char('h') | KeyCode::Left => {
                if self.accept(row) { Effect::Save(row) } else { Effect::None }
            }
            KeyCode::Char('q') | KeyCode::Char(',') => {
                if self.accept(row) { Effect::SaveAndClose(row) } else { Effect::Close }
            }
            KeyCode::Esc => {
                match self.before.take() {
                    Some(value) => self.values.insert(row.id.to_string(), value),
                    None => self.values.remove(row.id),
                };
                self.level = Level::Rows;
                Effect::Preview
            }
            _ => Effect::None,
        }
    }

    /// `direction` 1 = `k`/↑: a number grows, a choice moves up its list.
    fn change_value(&mut self, row: &Row, direction: i64) -> Effect {
        match row.kind {
            RowKind::Choice { options, .. } => {
                let index = if direction > 0 {
                    self.choice_index.saturating_sub(1)
                } else {
                    (self.choice_index + 1).min(options.len() - 1)
                };
                self.choice_index = index;
                self.values.insert(row.id.to_string(), json!(options[index]));
            }
            _ => row.step(&mut self.values, direction),
        }
        Effect::Preview
    }

    /// Leave the value level keeping the value; true when it differs from before.
    fn accept(&mut self, row: &Row) -> bool {
        self.level = Level::Rows;
        let before = self.before.take();
        self.values.get(row.id) != before.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows() -> impl Iterator<Item = &'static Row> {
        CATEGORIES.iter().flat_map(|c| c.rows.iter())
    }

    fn row(id: &str) -> &'static Row {
        rows().find(|r| r.id == id).unwrap()
    }

    /// A dialog state with the cursor on `id`, at the rows level.
    fn state_on(id: &str, values: Overrides) -> SettingsState {
        let mut state = SettingsState::new(values);
        for (ci, cat) in CATEGORIES.iter().enumerate() {
            if let Some(ri) = cat.rows.iter().position(|r| r.id == id) {
                state.category_index = ci;
                state.row_index = ri;
            }
        }
        state.level = Level::Rows;
        state
    }

    fn overrides(value: Value) -> Overrides {
        value.as_object().unwrap().clone()
    }

    #[test]
    fn all_categories_have_rows() {
        for cat in CATEGORIES {
            assert!(!cat.rows.is_empty(), "category '{}' has no rows", cat.name);
        }
    }

    #[test]
    fn row_table_and_struct_have_the_same_keys() {
        let schema = serde_json::to_value(schemars::schema_for!(WeztermOverrides)).unwrap();
        let mut fields: Vec<&str> = schema["properties"].as_object().unwrap().keys().map(String::as_str).collect();
        let mut ids: Vec<&str> = rows().map(|r| r.id).collect();
        fields.sort();
        ids.sort();
        assert_eq!(fields, ids);
    }

    #[test]
    fn every_row_default_fits_the_struct_and_survives_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut store = Store::load_at(&path).unwrap();
        for r in rows() {
            store.set(&r.file_key(), r.default_value()).unwrap();
        }
        let reloaded = Store::load_at(&path).unwrap();
        let values = overrides_of(reloaded.get());
        for r in rows() {
            assert_eq!(values.get(r.id), Some(&r.default_value()), "{}", r.id);
        }
    }

    #[test]
    fn missing_file_means_nothing_is_pushed() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::load_at(dir.path().join("config.toml")).unwrap();
        assert_eq!(to_wezterm_json(&overrides_of(store.get())), "{}");
    }

    #[test]
    fn only_set_keys_are_pushed_with_their_types() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "# mine\n[wezterm]\nwindow_decorations = \"TITLE | RESIZE\"\nfont_size = 14.0\nbold_brightens_ansi_colors = true\ninitial_cols = 120\n",
        )
        .unwrap();
        let store = Store::load_at(&path).unwrap();
        let pushed: Value = serde_json::from_str(&to_wezterm_json(&overrides_of(store.get()))).unwrap();
        assert_eq!(
            pushed,
            json!({
                "window_decorations": "TITLE | RESIZE",
                "font_size": 14.0,
                "bold_brightens_ansi_colors": true,
                "initial_cols": 120,
            })
        );
        // A float stays a float on the wire, as it was with settings.json.
        assert!(to_wezterm_json(&overrides_of(store.get())).contains("\"font_size\":14.0"));
    }

    #[test]
    fn saving_one_key_keeps_comments_and_unknown_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "# my note\nfuture_key = 1\n\n[wezterm]\nfont_size = 14.0 # big\n").unwrap();
        let mut store = Store::load_at(&path).unwrap();
        store.set("wezterm.font_size", json!(14.5)).unwrap();
        store.set("wezterm.enable_tab_bar", json!(false)).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# my note"));
        assert!(text.contains("future_key = 1"));
        assert!(text.contains("font_size = 14.5 # big"));
        assert!(text.contains("enable_tab_bar = false"));
    }

    #[test]
    fn value_returns_default_when_not_set() {
        assert_eq!(row("font_size").value(&Overrides::new()), json!(12.0));
        assert_eq!(row("font_size").value(&overrides(json!({"font_size": 16.0}))), json!(16.0));
    }

    #[test]
    fn display_formats_by_kind() {
        assert_eq!(row("font_size").display(&json!(14.0)), "14");
        assert_eq!(row("window_background_opacity").display(&json!(0.85)), "0.85");
        assert_eq!(row("initial_cols").display(&json!(80)), "80");
        assert_eq!(row("enable_tab_bar").display(&json!(false)), "OFF");
        assert_eq!(row("audible_bell").display(&json!("Disabled")), "Disabled");
    }

    #[test]
    fn choice_explanations_come_from_the_description() {
        let r = row("audible_bell");
        assert_eq!(r.explanation("Disabled"), Some("stay silent"));
        assert_eq!(r.explanation("SystemBeep"), Some("play the system sound on a bell"));
        assert_eq!(row("window_decorations").explanation("RESIZE"), Some("border only, no title bar"));
        assert_eq!(row("color_scheme").explanation("Nord"), None);
    }

    #[test]
    fn toggle_flips_in_place_and_saves() {
        let mut state = state_on("bold_brightens_ansi_colors", Overrides::new());
        let effect = state.handle_key(KeyCode::Enter);
        assert_eq!(effect, Effect::Save(row("bold_brightens_ansi_colors")));
        assert_eq!(state.values.get("bold_brightens_ansi_colors"), Some(&json!(false)));
        assert_eq!(state.level, Level::Rows);
        state.handle_key(KeyCode::Char('l'));
        assert_eq!(state.values.get("bold_brightens_ansi_colors"), Some(&json!(true)));
    }

    #[test]
    fn number_steps_live_and_saves_on_accept() {
        let mut state = state_on("font_size", overrides(json!({"font_size": 35.5})));
        assert_eq!(state.handle_key(KeyCode::Enter), Effect::None);
        assert_eq!(state.level, Level::Value);
        assert_eq!(state.handle_key(KeyCode::Char('k')), Effect::Preview);
        assert_eq!(state.values.get("font_size"), Some(&json!(36.0)));
        state.handle_key(KeyCode::Char('k'));
        assert_eq!(state.values.get("font_size"), Some(&json!(36.0))); // clamped to max
        assert_eq!(state.handle_key(KeyCode::Enter), Effect::Save(row("font_size")));
        assert_eq!(state.level, Level::Rows);
    }

    #[test]
    fn number_respects_min() {
        let mut state = state_on("font_size", overrides(json!({"font_size": 6.5})));
        state.handle_key(KeyCode::Enter);
        state.handle_key(KeyCode::Char('j'));
        state.handle_key(KeyCode::Char('j'));
        assert_eq!(state.values.get("font_size"), Some(&json!(6.0)));
    }

    #[test]
    fn int_steps_stay_integers() {
        let mut state = state_on("initial_cols", Overrides::new());
        state.handle_key(KeyCode::Enter);
        state.handle_key(KeyCode::Up);
        assert_eq!(state.values.get("initial_cols"), Some(&json!(90)));
        assert!(state.values["initial_cols"].is_i64());
    }

    #[test]
    fn esc_reverts_to_the_value_from_before() {
        let mut state = state_on("font_size", overrides(json!({"font_size": 14.0})));
        state.handle_key(KeyCode::Enter);
        state.handle_key(KeyCode::Char('k'));
        assert_eq!(state.handle_key(KeyCode::Esc), Effect::Preview);
        assert_eq!(state.values.get("font_size"), Some(&json!(14.0)));
        assert_eq!(state.level, Level::Rows);

        // A key that was not set is not set again afterwards.
        let mut state = state_on("max_fps", Overrides::new());
        state.handle_key(KeyCode::Enter);
        state.handle_key(KeyCode::Char('k'));
        state.handle_key(KeyCode::Esc);
        assert!(state.values.is_empty());
    }

    #[test]
    fn accepting_an_untouched_value_saves_nothing() {
        let mut state = state_on("max_fps", Overrides::new());
        state.handle_key(KeyCode::Enter);
        assert_eq!(state.handle_key(KeyCode::Char('h')), Effect::None);
        assert!(state.values.is_empty());
    }

    #[test]
    fn choice_unfolds_on_the_current_option_and_previews() {
        let mut state = state_on("audible_bell", overrides(json!({"audible_bell": "Disabled"})));
        state.handle_key(KeyCode::Enter);
        assert_eq!(state.level, Level::Value);
        assert_eq!(state.choice_index, 1);
        assert_eq!(state.handle_key(KeyCode::Char('k')), Effect::Preview);
        assert_eq!(state.values.get("audible_bell"), Some(&json!("SystemBeep")));
        state.handle_key(KeyCode::Char('k')); // stays on the first option
        assert_eq!(state.choice_index, 0);
        assert_eq!(state.handle_key(KeyCode::Enter), Effect::Save(row("audible_bell")));
    }

    #[test]
    fn q_closes_from_every_level() {
        let mut state = SettingsState::new(Overrides::new());
        assert_eq!(state.handle_key(KeyCode::Char(',')), Effect::Close);

        let mut state = state_on("font_size", Overrides::new());
        assert_eq!(state.handle_key(KeyCode::Char('q')), Effect::Close);

        let mut state = state_on("font_size", Overrides::new());
        state.handle_key(KeyCode::Enter);
        state.handle_key(KeyCode::Char('k'));
        assert_eq!(state.handle_key(KeyCode::Char('q')), Effect::SaveAndClose(row("font_size")));
    }

    #[test]
    fn levels_go_deeper_and_out() {
        let mut state = SettingsState::new(Overrides::new());
        state.handle_key(KeyCode::Char('j'));
        assert_eq!(state.category_index, 1);
        state.handle_key(KeyCode::Char('l'));
        assert_eq!(state.level, Level::Rows);
        state.handle_key(KeyCode::Char('j'));
        assert_eq!(state.row_index, 1);
        state.handle_key(KeyCode::Esc);
        assert_eq!(state.level, Level::Categories);
        state.handle_key(KeyCode::Char('k'));
        assert_eq!((state.category_index, state.row_index), (0, 0));
    }

    /// The companion plugin reads `config.toml` itself when WezTerm starts.
    /// This checks that its reader understands what `stack-settings` writes.
    #[test]
    fn lua_plugin_reads_what_the_dialog_writes() {
        if std::process::Command::new("lua").arg("-v").output().is_err() {
            eprintln!("skipped: no `lua` on PATH");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut store = Store::load_at(&path).unwrap();
        store.set("wezterm.font_size", json!(14.0)).unwrap();
        store.set("wezterm.window_background_opacity", json!(0.85)).unwrap();
        store.set("wezterm.window_decorations", json!("TITLE | RESIZE")).unwrap();
        store.set("wezterm.color_scheme", json!("Solarized (dark) (terminal.sexy)")).unwrap();
        store.set("wezterm.bold_brightens_ansi_colors", json!(true)).unwrap();
        store.set("wezterm.enable_scroll_bar", json!(false)).unwrap();
        store.set("wezterm.scrollback_lines", json!(3500)).unwrap();
        // What a person may add by hand around it.
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(
            &path,
            format!("{text}max_fps = 120 # smooth\nexit_behavior = 'Hold'\n\n[other]\nfont_size = 99\n"),
        )
        .unwrap();

        let plugin = concat!(env!("CARGO_MANIFEST_DIR"), "/plugin/init.lua");
        let script = r#"
            package.preload['wezterm'] = function() return { home_dir = '/nonexistent' } end
            local plugin = dofile(arg[1])
            local f = assert(io.open(arg[2], 'r'))
            local values = plugin.parse_overrides(f:read('a'))
            f:close()
            local keys = {}
            for k in pairs(values) do keys[#keys + 1] = k end
            table.sort(keys)
            for _, k in ipairs(keys) do
              print(k .. '|' .. type(values[k]) .. '|' .. tostring(values[k]))
            end
        "#;
        let script_path = dir.path().join("read.lua");
        std::fs::write(&script_path, script).unwrap();
        let out = std::process::Command::new("lua")
            .arg(&script_path)
            .arg(plugin)
            .arg(&path)
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let lines: Vec<&str> = std::str::from_utf8(&out.stdout).unwrap().lines().collect();
        assert_eq!(
            lines,
            [
                "bold_brightens_ansi_colors|boolean|true",
                "color_scheme|string|Solarized (dark) (terminal.sexy)",
                "enable_scroll_bar|boolean|false",
                "exit_behavior|string|Hold",
                "font_size|number|14.0",
                "max_fps|number|120",
                "scrollback_lines|number|3500",
                "window_background_opacity|number|0.85",
                "window_decorations|string|TITLE | RESIZE",
            ]
        );
    }
}
