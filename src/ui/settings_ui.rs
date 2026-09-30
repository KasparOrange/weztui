use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use super::{BG, BG2, FG, FG2, ORANGE, YELLOW};
use crate::settings::{Level, Row, RowKind, SettingsState, CATEGORIES};

const SLIDER_WIDTH: usize = 12;

pub fn render_settings(frame: &mut Frame, area: Rect, state: &SettingsState) {
    let [left_area, right_area] = Layout::horizontal([
        Constraint::Percentage(25),
        Constraint::Min(1),
    ])
    .areas(area);
    let [rows_area, desc_area] = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(5),
    ])
    .areas(right_area);

    render_categories(frame, left_area, state);
    if state.level == Level::Value && matches!(state.row().kind, RowKind::Choice { .. }) {
        render_choice(frame, rows_area, state);
    } else {
        render_rows(frame, rows_area, state);
    }
    render_description(frame, desc_area, state);
}

/// The pane that has the keyboard is bright, the other dim.
fn pane_block(title: String, focused: bool) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if focused { ORANGE } else { BG2 }))
        .title(title)
        .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(BG).fg(FG))
}

fn render_categories(frame: &mut Frame, area: Rect, state: &SettingsState) {
    let focused = state.level == Level::Categories;
    let items: Vec<ListItem> = CATEGORIES
        .iter()
        .enumerate()
        .map(|(i, cat)| {
            let is_selected = i == state.category_index;
            let style = if is_selected && focused {
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

    let list = List::new(items).block(pane_block(" Categories ".to_string(), focused));
    let mut list_state = ListState::default();
    list_state.select(Some(state.category_index));
    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_rows(frame: &mut Frame, area: Rect, state: &SettingsState) {
    let cat = &CATEGORIES[state.category_index];
    let focused = state.level != Level::Categories;

    let items: Vec<ListItem> = cat
        .rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let is_selected = i == state.row_index && focused;
            let adjusting = is_selected && state.level == Level::Value;
            let value = row.value(&state.values);

            let style = if is_selected {
                Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(FG2)
            };
            // A value set in config.toml stands out from one left to WezTerm.
            let value_style = if adjusting || state.values.contains_key(row.id) {
                Style::default().fg(YELLOW)
            } else {
                style
            };
            let value_text = if adjusting {
                slider(row, state)
            } else {
                row.display(&value)
            };

            let prefix = if is_selected { ">> " } else { "   " };
            let max_label = 28;
            let label = if row.label.len() > max_label {
                format!("{}...", &row.label[..max_label - 3])
            } else {
                row.label.to_string()
            };

            ListItem::new(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(format!("{:<width$}", label, width = max_label), style),
                Span::styled(format!("  {value_text}"), value_style),
            ]))
        })
        .collect();

    let list = List::new(items).block(pane_block(format!(" {} ", cat.name), focused));
    let mut list_state = ListState::default();
    if focused {
        list_state.select(Some(state.row_index));
    }
    frame.render_stateful_widget(list, area, &mut list_state);
}

/// A number being adjusted: `▾ ━━━●━━━━━━━━ 14 ▴`.
fn slider(row: &Row, state: &SettingsState) -> String {
    let knob = (row.fraction(&state.values) * (SLIDER_WIDTH - 1) as f64).round() as usize;
    let track: String = (0..SLIDER_WIDTH)
        .map(|i| if i == knob { '●' } else { '━' })
        .collect();
    format!("▾ {track} {} ▴", row.display(&row.value(&state.values)))
}

/// A choice unfolded into its options: ● marks the one from before.
fn render_choice(frame: &mut Frame, area: Rect, state: &SettingsState) {
    let row = state.row();
    let RowKind::Choice { options, .. } = row.kind else {
        return;
    };
    let current = state.before.clone().unwrap_or_else(|| row.default_value());

    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, &opt)| {
            let is_selected = i == state.choice_index;
            let style = if is_selected {
                Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(FG2)
            };
            let prefix = if is_selected { ">> " } else { "   " };
            let mark = if current.as_str() == Some(opt) { "● " } else { "  " };
            let mut spans = vec![Span::styled(format!("{prefix}{mark}{opt}"), style)];
            if let Some(explanation) = row.explanation(opt) {
                spans.push(Span::styled(format!("  {explanation}"), Style::default().fg(FG2)));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(items).block(pane_block(format!(" {} ", row.label), true));
    let mut list_state = ListState::default();
    list_state.select(Some(state.choice_index));
    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_description(frame: &mut Frame, area: Rect, state: &SettingsState) {
    let text = if state.level == Level::Categories {
        "WezTerm options weztui sets on top of your WezTerm config. Every change applies at once and is saved."
    } else {
        state.row().desc
    };
    let paragraph = Paragraph::new(text)
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(FG2))
        .block(pane_block(String::new(), false));
    frame.render_widget(paragraph, area);
}
