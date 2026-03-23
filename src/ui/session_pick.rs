use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui::Frame;

use super::{BG, BG2, FG, FG2, ORANGE, YELLOW};
use crate::session::SessionSummary;

pub fn render_session_pick(
    frame: &mut Frame,
    area: Rect,
    sessions: &[SessionSummary],
    selected_index: usize,
) {
    let items: Vec<ListItem> = sessions
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let style = if i == selected_index {
                Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(FG2)
            };
            let prefix = if i == selected_index { ">> " } else { "   " };
            let line = format!(
                "{}{:<20} {} window(s), {} tab(s)  [{}]",
                prefix, s.name, s.window_count, s.tab_count, s.saved_at
            );
            ListItem::new(Line::from(line)).style(style)
        })
        .collect();

    let count = sessions.len();
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BG2))
            .title(format!(
                " {} session{} ",
                count,
                if count == 1 { "" } else { "s" }
            ))
            .title_style(Style::default().fg(YELLOW).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(BG).fg(FG)),
    );

    let mut state = ListState::default();
    if !sessions.is_empty() {
        state.select(Some(selected_index));
    }
    frame.render_stateful_widget(list, area, &mut state);
}
