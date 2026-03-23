use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use super::{BG, AQUA, RED};

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
