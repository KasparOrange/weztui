use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui::Frame;

use super::{BG, BG2, FG, FG2, ORANGE, YELLOW};
use crate::settings::{
    self, SettingKind, SettingsPanel, SettingsState, CATEGORIES,
};

pub fn render_settings(frame: &mut Frame, area: Rect, state: &SettingsState) {
    let [left_area, right_area] = Layout::horizontal([
        Constraint::Percentage(25),
        Constraint::Min(1),
    ])
    .areas(area);

    render_categories(frame, left_area, state);
    render_setting_list(frame, right_area, state);
}

fn render_categories(frame: &mut Frame, area: Rect, state: &SettingsState) {
    let items: Vec<ListItem> = CATEGORIES
        .iter()
        .enumerate()
        .map(|(i, cat)| {
            let is_selected = i == state.category_index;
            let style = if is_selected && state.panel == SettingsPanel::Categories {
                Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default().fg(YELLOW)
            } else {
                Style::default().fg(FG2)
            };
            let prefix = if is_selected { ">> " } else { "   " };
            ListItem::new(Line::from(format!("{}{}", prefix, cat.name))).style(style)
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(
            if state.panel == SettingsPanel::Categories { ORANGE } else { BG2 }
        ))
        .title(" Categories ")
        .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(BG).fg(FG));

    let list = List::new(items).block(block);
    let mut list_state = ListState::default();
    list_state.select(Some(state.category_index));
    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_setting_list(frame: &mut Frame, area: Rect, state: &SettingsState) {
    // If selecting an enum value, show the options list instead
    if state.enum_selecting {
        render_enum_select(frame, area, state);
        return;
    }

    let cat = &CATEGORIES[state.category_index];

    let items: Vec<ListItem> = cat
        .settings
        .iter()
        .enumerate()
        .map(|(i, def)| {
            let is_selected = i == state.setting_index && state.panel == SettingsPanel::Settings;
            let val = settings::get_value(&state.values, def);
            let saved_val = settings::get_value(&state.saved_values, def);
            let is_modified = val != saved_val;

            let modified_marker = if is_modified { "*" } else { " " };

            // Value display with type indicator
            let val_display = if state.editing && is_selected {
                format!("{}_", state.edit_buffer)
            } else {
                format_value_with_indicator(&val, &def.kind)
            };

            let style = if is_selected {
                Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(FG2)
            };

            let prefix = if is_selected { ">> " } else { "   " };

            // Truncate label to fit
            let max_label = 28;
            let label = if def.label.len() > max_label {
                format!("{}...", &def.label[..max_label - 3])
            } else {
                def.label.to_string()
            };

            let line = Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(format!("{:<width$}", label, width = max_label), style),
                Span::styled(modified_marker, Style::default().fg(YELLOW)),
                Span::styled(format!(" {}", val_display), if is_modified {
                    Style::default().fg(YELLOW)
                } else {
                    style
                }),
            ]);

            ListItem::new(line)
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(
            if state.panel == SettingsPanel::Settings { ORANGE } else { BG2 }
        ))
        .title(format!(" {} ", cat.name))
        .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(BG).fg(FG));

    let list = List::new(items).block(block);
    let mut list_state = ListState::default();
    if state.panel == SettingsPanel::Settings {
        list_state.select(Some(state.setting_index));
    }
    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_enum_select(frame: &mut Frame, area: Rect, state: &SettingsState) {
    let cat = &CATEGORIES[state.category_index];
    let def = &cat.settings[state.setting_index];
    let options = if let SettingKind::Enum { options, .. } = &def.kind {
        options
    } else {
        return;
    };

    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, &opt)| {
            let is_selected = i == state.enum_select_index;
            let style = if is_selected {
                Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(FG2)
            };
            let prefix = if is_selected { ">> " } else { "   " };
            ListItem::new(Line::from(format!("{prefix}{opt}"))).style(style)
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ORANGE))
        .title(format!(" Select {} ", def.label))
        .title_style(Style::default().fg(YELLOW).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(BG).fg(FG));

    let list = List::new(items).block(block);
    let mut list_state = ListState::default();
    list_state.select(Some(state.enum_select_index));
    frame.render_stateful_widget(list, area, &mut list_state);
}

fn format_value_with_indicator(val: &settings::SettingValue, kind: &SettingKind) -> String {
    match val {
        settings::SettingValue::Bool(b) => {
            if *b { "[ON]".to_string() } else { "[OFF]".to_string() }
        }
        settings::SettingValue::Float(f) => format!("{:.1}  [+/-]", f),
        settings::SettingValue::Int(i) => format!("{}  [+/-]", i),
        settings::SettingValue::Str(s) => {
            if matches!(kind, SettingKind::Enum { .. }) {
                format!("{}  [Enter]", s)
            } else {
                s.clone()
            }
        }
    }
}
