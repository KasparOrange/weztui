use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use tui_tree_widget::Tree;

use super::{BG, BG1, BG2, FG, FG2, ORANGE, RED};
use crate::app::{build_move_preview, build_tree_items, App, Mode, NodeId};

pub fn render_tree(frame: &mut Frame, area: Rect, app: &mut App) {
    let in_move_mode = matches!(app.mode, Mode::Move { .. });

    // In move mode, build a preview of the tree with the item moved
    let (preview_windows, _ghost_tab_id) = if let Mode::Move { ref grabbed, .. } = app.mode {
        // Determine target window from current selection
        let target_window_id = match app.tree_state.selected().last() {
            Some(NodeId::Window(id)) => Some(*id),
            Some(NodeId::Tab(tab_id)) => {
                app.windows.iter()
                    .find(|w| w.tabs.iter().any(|t| t.tab_id == *tab_id))
                    .map(|w| w.window_id)
            }
            Some(NodeId::Pane(pane_id)) => {
                app.windows.iter()
                    .find(|w| w.tabs.iter().flat_map(|t| &t.panes).any(|p| p.pane_id == *pane_id))
                    .map(|w| w.window_id)
            }
            _ => None,
        };

        if let Some(target_id) = target_window_id {
            let (preview, ghost) = build_move_preview(&app.windows, grabbed, target_id);
            (Some(preview), ghost)
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    let tree_items = if let Some(ref preview) = preview_windows {
        build_tree_items(preview, app.current_pane_id)
    } else {
        build_tree_items(&app.windows, app.current_pane_id)
    };

    // Title
    let title = if let Mode::Move { ref grabbed_label, .. } = app.mode {
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

    let tree_widget = Tree::new(&tree_items)
        .expect("tree items have unique ids")
        .block(block)
        .style(Style::default().fg(FG2).bg(BG))
        .highlight_style(highlight_style)
        .highlight_symbol(">> ")
        .node_closed_symbol("▶ ")
        .node_open_symbol("▼ ")
        .node_no_children_symbol("  ");

    frame.render_stateful_widget(tree_widget, area, &mut app.tree_state);
}
