use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::{BG1, FG2, ORANGE, AQUA};
use crate::app::App;

pub fn render_status(frame: &mut Frame, area: Rect, app: &App) {
    let info = app.selected_info();
    let hints = "j/k:navigate  Enter:expand  q:quit";

    let status_line = Line::from(vec![
        Span::styled(
            format!(" {info} "),
            Style::default()
                .fg(ORANGE)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("│", Style::default().fg(FG2)),
        Span::styled(
            format!(" {hints} "),
            Style::default().fg(AQUA),
        ),
    ]);

    let status = Paragraph::new(status_line).style(Style::default().bg(BG1));
    frame.render_widget(status, area);
}
