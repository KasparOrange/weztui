use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use tui_tree_widget::Tree;

use super::{BG, BG1, BG2, FG, FG2, ORANGE, RED};
use crate::app::{build_tree_items, App, Mode, NodeId};

pub fn render_tree(frame: &mut Frame, area: Rect, app: &mut App) {
    let tree_items = build_tree_items(&app.windows, app.current_pane_id);

    let in_move_mode = matches!(app.mode, Mode::Move { .. });

    // In move mode, show what's being grabbed in the title
    let title = if let Mode::Move { ref grabbed } = app.mode {
        let grabbed_label = match grabbed {
            NodeId::Pane(id) => {
                app.windows.iter()
                    .flat_map(|w| &w.tabs)
                    .flat_map(|t| &t.panes)
                    .find(|p| p.pane_id == *id)
                    .map(|p| p.title.clone())
                    .unwrap_or_else(|| format!("Pane {id}"))
            }
            NodeId::Tab(id) => {
                app.windows.iter()
                    .flat_map(|w| &w.tabs)
                    .find(|t| t.tab_id == *id)
                    .and_then(|t| t.title.clone())
                    .unwrap_or_else(|| format!("Tab {id}"))
            }
            _ => String::new(),
        };
        format!(" Moving: {} ", grabbed_label)
    } else {
        " Windows ".to_string()
    };

    let title_color = if in_move_mode { RED } else { ORANGE };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if in_move_mode { RED } else { BG2 }))
        .title(title)
        .title_style(Style::default().fg(title_color).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(BG).fg(FG));

    if tree_items.is_empty() {
        let message = Paragraph::new("  No windows found. Is WezTerm running?")
            .style(Style::default().fg(FG2).bg(BG))
            .block(block);
        frame.render_widget(message, area);
        return;
    }

    // In move mode: red highlight shows drop target
    let highlight_style = if in_move_mode {
        Style::default()
            .fg(FG)
            .bg(RED)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(ORANGE)
            .bg(BG1)
            .add_modifier(Modifier::BOLD)
    };

    let highlight_symbol = if in_move_mode { ">> " } else { ">> " };

    let tree_widget = Tree::new(&tree_items)
        .expect("tree items have unique ids")
        .block(block)
        .style(Style::default().fg(FG2).bg(BG))
        .highlight_style(highlight_style)
        .highlight_symbol(highlight_symbol)
        .node_closed_symbol("▶ ")
        .node_open_symbol("▼ ")
        .node_no_children_symbol("  ");

    frame.render_stateful_widget(tree_widget, area, &mut app.tree_state);
}
