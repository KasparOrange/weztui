use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use super::{BG, BG1, FG, FG2, ORANGE, RED, YELLOW, AQUA};

pub fn render_move_popup(
    frame: &mut Frame,
    area: Rect,
    choices: &[(u64, String)],
    selected: usize,
) {
    let height = (choices.len() as u16 + 4).min(area.height.saturating_sub(4));
    let popup_area = centered_rect(50, height, area);
    frame.render_widget(Clear, popup_area);

    let items: Vec<ListItem> = choices
        .iter()
        .enumerate()
        .map(|(i, (_, label))| {
            let style = if i == selected {
                Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(FG)
            };
            ListItem::new(Line::from(format!("  {label}"))).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BG1))
            .title(" Move to Window ")
            .title_style(Style::default().fg(YELLOW).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(BG).fg(FG)),
    );

    let mut state = ListState::default();
    state.select(Some(selected));
    frame.render_stateful_widget(list, popup_area, &mut state);

    // Hint line below the popup
    let hint_area = Rect {
        x: popup_area.x,
        y: popup_area.y + popup_area.height,
        width: popup_area.width,
        height: 1.min(area.height.saturating_sub(popup_area.y + popup_area.height)),
    };
    if hint_area.height > 0 {
        let hint = Line::from(" j/k:select  Enter:confirm  Esc:cancel")
            .style(Style::default().fg(FG2));
        frame.render_widget(hint, hint_area);
    }
}

pub fn render_confirm_popup(frame: &mut Frame, area: Rect, label: &str) {
    let popup_area = centered_rect(50, 5, area);
    frame.render_widget(Clear, popup_area);

    let text = format!("{label}\n\n y: yes   n/Esc: cancel");
    let paragraph = Paragraph::new(text)
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(RED))
                .title(" Confirm ")
                .title_style(Style::default().fg(RED).add_modifier(Modifier::BOLD))
                .style(Style::default().bg(BG)),
        )
        .style(Style::default().fg(AQUA));

    frame.render_widget(paragraph, popup_area);
}

pub fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .split(area);
    Layout::horizontal([Constraint::Percentage(percent_x)])
        .flex(Flex::Center)
        .split(vertical[0])[0]
}
