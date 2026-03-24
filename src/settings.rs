use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use color_eyre::Result;
use serde::{Deserialize, Serialize};

// -- Types --

#[derive(Debug, Clone)]
pub enum SettingKind {
    Bool { default: bool },
    Float { default: f64, min: f64, max: f64, step: f64 },
    Int { default: i64, min: i64, max: i64, step: i64 },
    Enum { options: &'static [&'static str], default_index: usize },
}

#[derive(Debug, Clone)]
pub struct SettingDef {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: SettingKind,
}

#[derive(Debug, Clone)]
pub struct SettingsCategory {
    pub name: &'static str,
    pub settings: &'static [SettingDef],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SettingValue {
    Bool(bool),
    Float(f64),
    Int(i64),
    Str(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum SettingsPanel {
    Categories,
    Settings,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SettingsState {
    pub category_index: usize,
    pub setting_index: usize,
    pub panel: SettingsPanel,
    pub values: HashMap<String, SettingValue>,
    pub saved_values: HashMap<String, SettingValue>,
    pub editing: bool,
    pub edit_buffer: String,
    pub edit_cursor: usize,
}

// -- Static Catalog --

pub static CATEGORIES: &[SettingsCategory] = &[
    SettingsCategory {
        name: "Font & Text",
        settings: &[
            SettingDef { key: "font_size", label: "Font Size", kind: SettingKind::Float { default: 12.0, min: 6.0, max: 36.0, step: 0.5 } },
            SettingDef { key: "line_height", label: "Line Height", kind: SettingKind::Float { default: 1.0, min: 0.5, max: 2.0, step: 0.1 } },
            SettingDef { key: "cell_width", label: "Cell Width", kind: SettingKind::Float { default: 1.0, min: 0.5, max: 2.0, step: 0.1 } },
            SettingDef { key: "bold_brightens_ansi_colors", label: "Bold Brightens Colors", kind: SettingKind::Bool { default: true } },
            SettingDef { key: "adjust_window_size_when_changing_font_size", label: "Resize Window on Font Change", kind: SettingKind::Bool { default: true } },
        ],
    },
    SettingsCategory {
        name: "Colors & Themes",
        settings: &[
            SettingDef { key: "color_scheme", label: "Color Scheme", kind: SettingKind::Enum {
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
            SettingDef { key: "window_background_opacity", label: "Window Opacity", kind: SettingKind::Float { default: 1.0, min: 0.0, max: 1.0, step: 0.05 } },
            SettingDef { key: "text_background_opacity", label: "Text Background Opacity", kind: SettingKind::Float { default: 1.0, min: 0.0, max: 1.0, step: 0.05 } },
        ],
    },
    SettingsCategory {
        name: "Window",
        settings: &[
            SettingDef { key: "window_decorations", label: "Window Decorations", kind: SettingKind::Enum {
                options: &["TITLE | RESIZE", "RESIZE", "NONE", "INTEGRATED_BUTTONS | RESIZE"],
                default_index: 0,
            }},
            SettingDef { key: "window_close_confirmation", label: "Close Confirmation", kind: SettingKind::Enum {
                options: &["AlwaysPrompt", "NeverPrompt"],
                default_index: 0,
            }},
            SettingDef { key: "native_macos_fullscreen_mode", label: "macOS Native Fullscreen", kind: SettingKind::Bool { default: false } },
            SettingDef { key: "initial_cols", label: "Initial Columns", kind: SettingKind::Int { default: 80, min: 20, max: 400, step: 10 } },
            SettingDef { key: "initial_rows", label: "Initial Rows", kind: SettingKind::Int { default: 24, min: 10, max: 200, step: 5 } },
        ],
    },
    SettingsCategory {
        name: "Tab Bar",
        settings: &[
            SettingDef { key: "enable_tab_bar", label: "Enable Tab Bar", kind: SettingKind::Bool { default: true } },
            SettingDef { key: "use_fancy_tab_bar", label: "Fancy Tab Bar", kind: SettingKind::Bool { default: true } },
            SettingDef { key: "hide_tab_bar_if_only_one_tab", label: "Hide If Single Tab", kind: SettingKind::Bool { default: false } },
            SettingDef { key: "tab_bar_at_bottom", label: "Tab Bar at Bottom", kind: SettingKind::Bool { default: false } },
            SettingDef { key: "show_tab_index_in_tab_bar", label: "Show Tab Index", kind: SettingKind::Bool { default: true } },
        ],
    },
    SettingsCategory {
        name: "Cursor",
        settings: &[
            SettingDef { key: "default_cursor_style", label: "Cursor Style", kind: SettingKind::Enum {
                options: &["SteadyBlock", "BlinkingBlock", "SteadyUnderline", "BlinkingUnderline", "SteadyBar", "BlinkingBar"],
                default_index: 0,
            }},
            SettingDef { key: "cursor_blink_rate", label: "Blink Rate (ms)", kind: SettingKind::Int { default: 800, min: 0, max: 2000, step: 100 } },
            SettingDef { key: "force_reverse_video_cursor", label: "Reverse Video Cursor", kind: SettingKind::Bool { default: false } },
        ],
    },
    SettingsCategory {
        name: "Scrollback",
        settings: &[
            SettingDef { key: "scrollback_lines", label: "Scrollback Lines", kind: SettingKind::Int { default: 3500, min: 0, max: 100000, step: 500 } },
            SettingDef { key: "enable_scroll_bar", label: "Show Scrollbar", kind: SettingKind::Bool { default: false } },
            SettingDef { key: "hide_mouse_cursor_when_typing", label: "Hide Mouse When Typing", kind: SettingKind::Bool { default: true } },
            SettingDef { key: "pane_focus_follows_mouse", label: "Focus Follows Mouse", kind: SettingKind::Bool { default: false } },
        ],
    },
    SettingsCategory {
        name: "Behavior",
        settings: &[
            SettingDef { key: "exit_behavior", label: "Exit Behavior", kind: SettingKind::Enum {
                options: &["Close", "Hold", "CloseOnCleanExit"],
                default_index: 0,
            }},
            SettingDef { key: "audible_bell", label: "Audible Bell", kind: SettingKind::Enum {
                options: &["SystemBeep", "Disabled"],
                default_index: 0,
            }},
            SettingDef { key: "automatically_reload_config", label: "Auto-Reload Config", kind: SettingKind::Bool { default: true } },
            SettingDef { key: "max_fps", label: "Max FPS", kind: SettingKind::Int { default: 60, min: 10, max: 240, step: 10 } },
        ],
    },
];

// -- Value Helpers --

pub fn get_value(values: &HashMap<String, SettingValue>, def: &SettingDef) -> SettingValue {
    if let Some(v) = values.get(def.key) {
        return v.clone();
    }
    match &def.kind {
        SettingKind::Bool { default } => SettingValue::Bool(*default),
        SettingKind::Float { default, .. } => SettingValue::Float(*default),
        SettingKind::Int { default, .. } => SettingValue::Int(*default),
        SettingKind::Enum { options, default_index } => SettingValue::Str(options[*default_index].to_string()),
    }
}

pub fn display_value(val: &SettingValue) -> String {
    match val {
        SettingValue::Bool(b) => if *b { "ON".to_string() } else { "OFF".to_string() },
        SettingValue::Float(f) => format!("{:.1}", f),
        SettingValue::Int(i) => i.to_string(),
        SettingValue::Str(s) => s.clone(),
    }
}

pub fn toggle_bool(values: &mut HashMap<String, SettingValue>, def: &SettingDef) {
    let current = match get_value(values, def) {
        SettingValue::Bool(b) => b,
        _ => return,
    };
    values.insert(def.key.to_string(), SettingValue::Bool(!current));
}

pub fn increment(values: &mut HashMap<String, SettingValue>, def: &SettingDef) {
    match &def.kind {
        SettingKind::Float { max, step, .. } => {
            if let SettingValue::Float(v) = get_value(values, def) {
                let new = (v + step).min(*max);
                values.insert(def.key.to_string(), SettingValue::Float((new * 100.0).round() / 100.0));
            }
        }
        SettingKind::Int { max, step, .. } => {
            if let SettingValue::Int(v) = get_value(values, def) {
                let new = (v + step).min(*max);
                values.insert(def.key.to_string(), SettingValue::Int(new));
            }
        }
        _ => {}
    }
}

pub fn decrement(values: &mut HashMap<String, SettingValue>, def: &SettingDef) {
    match &def.kind {
        SettingKind::Float { min, step, .. } => {
            if let SettingValue::Float(v) = get_value(values, def) {
                let new = (v - step).max(*min);
                values.insert(def.key.to_string(), SettingValue::Float((new * 100.0).round() / 100.0));
            }
        }
        SettingKind::Int { min, step, .. } => {
            if let SettingValue::Int(v) = get_value(values, def) {
                let new = (v - step).max(*min);
                values.insert(def.key.to_string(), SettingValue::Int(new));
            }
        }
        _ => {}
    }
}

pub fn cycle_enum(values: &mut HashMap<String, SettingValue>, def: &SettingDef) {
    if let SettingKind::Enum { options, .. } = &def.kind {
        let current = match get_value(values, def) {
            SettingValue::Str(s) => s,
            _ => return,
        };
        let idx = options.iter().position(|&o| o == current).unwrap_or(0);
        let next = (idx + 1) % options.len();
        values.insert(def.key.to_string(), SettingValue::Str(options[next].to_string()));
    }
}

// -- JSON for WezTerm config overrides --

pub fn to_wezterm_json(values: &HashMap<String, SettingValue>) -> String {
    let mut map = serde_json::Map::new();
    for (key, val) in values {
        let json_val = match val {
            SettingValue::Bool(b) => serde_json::Value::Bool(*b),
            SettingValue::Float(f) => serde_json::json!(*f),
            SettingValue::Int(i) => serde_json::json!(*i),
            SettingValue::Str(s) => serde_json::Value::String(s.clone()),
        };
        // Handle nested keys like "window_padding.left"
        if let Some(dot) = key.find('.') {
            let (parent, child) = key.split_at(dot);
            let child = &child[1..]; // skip the dot
            let obj = map.entry(parent.to_string())
                .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
            if let serde_json::Value::Object(m) = obj {
                m.insert(child.to_string(), json_val);
            }
        } else {
            map.insert(key.clone(), json_val);
        }
    }
    serde_json::Value::Object(map).to_string()
}

// -- Persistence --

fn settings_file() -> PathBuf {
    let dir = std::env::var("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            std::path::PathBuf::from(home).join(".config")
        })
        .join("weztui");
    let _ = fs::create_dir_all(&dir);
    dir.join("settings.json")
}

pub fn load_settings() -> HashMap<String, SettingValue> {
    let path = settings_file();
    if !path.exists() {
        return HashMap::new();
    }
    fs::read_to_string(&path)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

pub fn save_settings(values: &HashMap<String, SettingValue>) -> Result<()> {
    let path = settings_file();
    let json = serde_json::to_string_pretty(values)?;
    fs::write(&path, json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn font_size_def() -> &'static SettingDef {
        &CATEGORIES[0].settings[0] // font_size
    }

    fn bool_def() -> &'static SettingDef {
        &CATEGORIES[0].settings[3] // bold_brightens_ansi_colors
    }

    fn enum_def() -> &'static SettingDef {
        &CATEGORIES[4].settings[0] // default_cursor_style
    }

    #[test]
    fn get_value_returns_default_when_empty() {
        let values = HashMap::new();
        let val = get_value(&values, font_size_def());
        assert_eq!(val, SettingValue::Float(12.0));
    }

    #[test]
    fn get_value_returns_override() {
        let mut values = HashMap::new();
        values.insert("font_size".to_string(), SettingValue::Float(16.0));
        let val = get_value(&values, font_size_def());
        assert_eq!(val, SettingValue::Float(16.0));
    }

    #[test]
    fn toggle_bool_flips() {
        let mut values = HashMap::new();
        toggle_bool(&mut values, bool_def());
        assert_eq!(values.get("bold_brightens_ansi_colors"), Some(&SettingValue::Bool(false)));
        toggle_bool(&mut values, bool_def());
        assert_eq!(values.get("bold_brightens_ansi_colors"), Some(&SettingValue::Bool(true)));
    }

    #[test]
    fn increment_respects_max() {
        let mut values = HashMap::new();
        values.insert("font_size".to_string(), SettingValue::Float(35.5));
        increment(&mut values, font_size_def());
        assert_eq!(values.get("font_size"), Some(&SettingValue::Float(36.0)));
        increment(&mut values, font_size_def());
        assert_eq!(values.get("font_size"), Some(&SettingValue::Float(36.0))); // clamped
    }

    #[test]
    fn decrement_respects_min() {
        let mut values = HashMap::new();
        values.insert("font_size".to_string(), SettingValue::Float(6.5));
        decrement(&mut values, font_size_def());
        assert_eq!(values.get("font_size"), Some(&SettingValue::Float(6.0)));
        decrement(&mut values, font_size_def());
        assert_eq!(values.get("font_size"), Some(&SettingValue::Float(6.0))); // clamped
    }

    #[test]
    fn cycle_enum_wraps() {
        let mut values = HashMap::new();
        // Default is "SteadyBlock" (index 0)
        cycle_enum(&mut values, enum_def());
        assert_eq!(values.get("default_cursor_style"), Some(&SettingValue::Str("BlinkingBlock".to_string())));
        // Cycle through all 6, should wrap
        for _ in 0..5 {
            cycle_enum(&mut values, enum_def());
        }
        assert_eq!(values.get("default_cursor_style"), Some(&SettingValue::Str("SteadyBlock".to_string())));
    }

    #[test]
    fn to_wezterm_json_valid() {
        let mut values = HashMap::new();
        values.insert("font_size".to_string(), SettingValue::Float(14.0));
        values.insert("enable_tab_bar".to_string(), SettingValue::Bool(false));
        let json = to_wezterm_json(&values);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["font_size"], 14.0);
        assert_eq!(parsed["enable_tab_bar"], false);
    }

    #[test]
    fn save_load_roundtrip() {
        let mut values = HashMap::new();
        values.insert("font_size".to_string(), SettingValue::Float(14.0));
        values.insert("enable_tab_bar".to_string(), SettingValue::Bool(false));

        let tmp = std::env::temp_dir().join("weztui-test-settings.json");
        let json = serde_json::to_string_pretty(&values).unwrap();
        fs::write(&tmp, &json).unwrap();

        let loaded: HashMap<String, SettingValue> =
            serde_json::from_str(&fs::read_to_string(&tmp).unwrap()).unwrap();
        assert_eq!(values, loaded);
        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn all_categories_have_settings() {
        for cat in CATEGORIES {
            assert!(!cat.settings.is_empty(), "category '{}' has no settings", cat.name);
        }
    }
}
