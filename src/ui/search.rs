use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use super::{BG, BG1, BG2, FG, FG2, ORANGE, YELLOW, AQUA};
use crate::search::{SearchEntry, SearchResult};

pub fn render_search(
    frame: &mut Frame,
    area: Rect,
    query: &str,
    cursor: usize,
    entries: &[SearchEntry],
    results: &[SearchResult],
    selected_index: usize,
) {
    let [input_area, results_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .areas(area);

    // Search input
    let prefix = " / ";
    let input_line = Line::from(vec![
        Span::styled(prefix, Style::default().fg(YELLOW).add_modifier(Modifier::BOLD)),
        Span::styled(query, Style::default().fg(FG)),
    ]);
    frame.render_widget(
        Paragraph::new(input_line).style(Style::default().bg(BG1)),
        input_area,
    );

    // Cursor
    let cursor_x = input_area.x + prefix.len() as u16 + cursor as u16;
    if cursor_x < input_area.x + input_area.width {
        frame.set_cursor_position((cursor_x, input_area.y));
    }

    // Results
    let items: Vec<ListItem> = results
        .iter()
        .enumerate()
        .map(|(i, result)| {
            let entry = &entries[result.entry_index];
            let style = if i == selected_index {
                Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(FG2)
            };
            let prefix = if i == selected_index { ">> " } else { "   " };
            ListItem::new(Line::from(format!("{}{}", prefix, entry.display))).style(style)
        })
        .collect();

    let count = results.len();
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(BG2))
            .title(format!(
                " {} result{} ",
                count,
                if count == 1 { "" } else { "s" }
            ))
            .title_style(Style::default().fg(FG2))
            .style(Style::default().bg(BG)),
    );

    let mut state = ListState::default();
    if !results.is_empty() {
        state.select(Some(selected_index));
    }
    frame.render_stateful_widget(list, results_area, &mut state);
}
