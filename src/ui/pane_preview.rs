use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use super::{BG, BG2, FG, FG2, ORANGE};

pub fn render_pane_preview(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    content: &str,
) {
    let display_title = if title.is_empty() {
        " Preview ".to_string()
    } else {
        format!(" {} ", title)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BG2))
        .title(display_title)
        .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(BG).fg(FG));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    // Trim trailing blank lines, take only what fits
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return;
    }
    let trimmed: Vec<&str> = {
        let last_nonempty = lines.iter().rposition(|l| !l.trim().is_empty()).unwrap_or(0);
        lines[..=last_nonempty].to_vec()
    };

    // Take the last N lines that fit (show the bottom of the terminal, which is most relevant)
    let visible_lines: Vec<&str> = if trimmed.len() > inner.height as usize {
        trimmed[trimmed.len() - inner.height as usize..].to_vec()
    } else {
        trimmed
    };

    let text = visible_lines.join("\n");
    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(FG2))
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, inner);
}
