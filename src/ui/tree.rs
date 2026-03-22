use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders};
use ratatui::Frame;
use tui_tree_widget::Tree;

use super::{BG, BG1, BG2, FG, FG2, ORANGE};
use crate::app::{build_tree_items, App};

pub fn render_tree(frame: &mut Frame, area: Rect, app: &mut App) {
    let tree_items = build_tree_items(&app.windows, app.current_pane_id);

    let tree_widget = Tree::new(&tree_items)
        .expect("tree items have unique ids")
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BG2))
                .title(" Windows ")
                .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD))
                .style(Style::default().bg(BG).fg(FG)),
        )
        .style(Style::default().fg(FG2).bg(BG))
        .highlight_style(
            Style::default()
                .fg(ORANGE)
                .bg(BG1)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ")
        .node_closed_symbol("▶ ")
        .node_open_symbol("▼ ")
        .node_no_children_symbol("  ");

    frame.render_stateful_widget(tree_widget, area, &mut app.tree_state);
}
