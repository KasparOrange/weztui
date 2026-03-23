use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::{BG1, FG, FG2, ORANGE, AQUA, GREEN, RED, YELLOW};
use crate::app::{App, Mode};

pub fn render_status(frame: &mut Frame, area: Rect, app: &App) {
    match &app.mode {
        Mode::Normal => render_normal_status(frame, area, app),
        Mode::Rename { input, cursor } => render_rename_status(frame, area, input, *cursor),
        Mode::Move { .. } => render_hint_status(frame, area, "j/k:navigate to target window  Enter/m:drop  Esc:cancel"),
        Mode::Confirm { .. } => render_hint_status(frame, area, "y:confirm  n/Esc:cancel"),
        Mode::Search { direct_launch, .. } => {
            let back = if *direct_launch { "Esc:quit" } else { "Esc:back" };
            render_hint_status(frame, area, &format!("j/k:select  Enter:focus  {back}"));
        }
        Mode::Help => render_hint_status(frame, area, "Press any key to close"),
        Mode::SessionPick { .. } => render_hint_status(frame, area, "j/k:select  Enter:restore  x:delete  Esc:back"),
    }
}

fn render_normal_status(frame: &mut Frame, area: Rect, app: &App) {
    let hints = "Enter:focus  r:rename  m:move  x:close  /:find  s:sessions  ?:help  q:quit";

    let status_line = if let Some(ref msg) = app.status_message {
        let color = if msg.is_error { RED } else { GREEN };
        Line::from(vec![
            Span::styled(
                format!(" {} ", msg.text),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled("│", Style::default().fg(FG2)),
            Span::styled(format!(" {hints} "), Style::default().fg(AQUA)),
        ])
    } else {
        let info = app.selected_info();
        Line::from(vec![
            Span::styled(
                format!(" {info} "),
                Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
            ),
            Span::styled("│", Style::default().fg(FG2)),
            Span::styled(format!(" {hints} "), Style::default().fg(AQUA)),
        ])
    };

    let status = Paragraph::new(status_line).style(Style::default().bg(BG1));
    frame.render_widget(status, area);
}

fn render_rename_status(frame: &mut Frame, area: Rect, input: &str, cursor: usize) {
    let prefix = " Rename: ";
    let status_line = Line::from(vec![
        Span::styled(prefix, Style::default().fg(YELLOW).add_modifier(Modifier::BOLD)),
        Span::styled(input, Style::default().fg(FG)),
        Span::styled(" │ Enter:confirm  Esc:cancel", Style::default().fg(AQUA)),
    ]);

    let status = Paragraph::new(status_line).style(Style::default().bg(BG1));
    frame.render_widget(status, area);

    // Show text cursor
    let cursor_x = area.x + prefix.len() as u16 + cursor as u16;
    let cursor_y = area.y;
    if cursor_x < area.x + area.width {
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}

fn render_hint_status(frame: &mut Frame, area: Rect, hints: &str) {
    let status_line = Line::from(vec![
        Span::styled(format!(" {hints} "), Style::default().fg(AQUA)),
    ]);
    let status = Paragraph::new(status_line).style(Style::default().bg(BG1));
    frame.render_widget(status, area);
}
