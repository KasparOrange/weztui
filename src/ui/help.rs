use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use super::popup::centered_rect;
use super::{BG, FG, ORANGE, YELLOW};

const HELP_TEXT: &str = "\
 Navigation
   j / Down       Move down
   k / Up         Move up
   h / Left       Collapse / parent
   l / Right      Expand / child
   Home / End     First / last item

 Actions
   Enter / f      Focus pane (quit) / toggle expand
   r              Rename tab or window
   m              Move tab or pane to another window
   x              Close (with confirmation)

 Modes
   /              Fuzzy search
   s              Session picker (restore / delete)
   ?              This help screen
   q / Esc        Quit";

pub fn render_help(frame: &mut Frame, area: Rect) {
    let height = (HELP_TEXT.lines().count() as u16 + 2).min(area.height.saturating_sub(2));
    let popup_area = centered_rect(60, height, area);
    frame.render_widget(Clear, popup_area);

    let paragraph = Paragraph::new(HELP_TEXT)
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ORANGE))
                .title(" Keybindings ")
                .title_style(Style::default().fg(YELLOW).add_modifier(Modifier::BOLD))
                .style(Style::default().bg(BG)),
        )
        .style(Style::default().fg(FG));

    frame.render_widget(paragraph, popup_area);
}
