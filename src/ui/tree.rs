use std::collections::HashMap;

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use super::{BG, BG1, BG2, FG, FG2, ORANGE, RED};
use crate::app::{build_tree_items, App, Mode, NodeId};

/// Build a label map from the model data (same labels as build_tree_items uses).
fn build_label_map(app: &App) -> HashMap<Vec<NodeId>, String> {
    let mut labels = HashMap::new();
    for w in &app.windows {
        let win_id = vec![NodeId::Window(w.window_id)];
        let win_label = format!(
            "{}  ({})",
            w.title.as_deref().unwrap_or(&format!("Window {}", w.window_id)),
            w.window_id,
        );

        // Single-tab single-pane window → flat label
        if w.tabs.len() == 1 && w.tabs[0].panes.len() == 1 {
            let tab = &w.tabs[0];
            let pane = &tab.panes[0];
            let active = if tab.panes.iter().any(|p| p.is_active) { "● " } else { "" };
            let cwd = pane.cwd.as_ref().and_then(|c| c.rsplit('/').next()).unwrap_or("~");
            let marker = if Some(pane.pane_id) == app.current_pane_id { " *" } else { "" };
            labels.insert(win_id, format!(
                "{}{}  — {} [{}]{}",
                active,
                w.title.as_deref().unwrap_or(&format!("Window {}", w.window_id)),
                pane.title, cwd, marker,
            ));
            continue;
        }

        labels.insert(win_id.clone(), win_label);

        for tab in &w.tabs {
            let tab_id = vec![NodeId::Window(w.window_id), NodeId::Tab(tab.tab_id)];
            let active = if tab.panes.iter().any(|p| p.is_active) { "● " } else { "" };
            let tab_fallback = format!("Tab {}", tab.tab_id);
            let tab_title = tab.title.as_deref().unwrap_or(&tab_fallback);

            if tab.panes.len() == 1 {
                let pane = &tab.panes[0];
                let cwd = pane.cwd.as_ref().and_then(|c| c.rsplit('/').next()).unwrap_or("~");
                let marker = if Some(pane.pane_id) == app.current_pane_id { " *" } else { "" };
                labels.insert(tab_id, format!(
                    "{}{} — {} [{}]{}",
                    active, tab_title, pane.title, cwd, marker,
                ));
            } else {
                labels.insert(tab_id.clone(), format!(
                    "{}{}  ({} panes)",
                    active, tab_title, tab.panes.len(),
                ));
                for pane in &tab.panes {
                    let pane_path = vec![
                        NodeId::Window(w.window_id),
                        NodeId::Tab(tab.tab_id),
                        NodeId::Pane(pane.pane_id),
                    ];
                    let cwd = pane.cwd.as_ref().and_then(|c| c.rsplit('/').next()).unwrap_or("~");
                    let marker = if Some(pane.pane_id) == app.current_pane_id { " *" } else { "" };
                    labels.insert(pane_path, format!("{} [{}]{}", pane.title, cwd, marker));
                }
            }
        }
    }
    labels
}

pub fn render_tree(frame: &mut Frame, area: Rect, app: &mut App) {
    let tree_items = build_tree_items(&app.windows, app.current_pane_id);
    let in_move_mode = matches!(app.mode, Mode::Move { .. });

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

    // Render the Tree widget invisibly to update TreeState's internal tracking
    // (needed for key_up/key_down navigation to work)
    let hidden_tree = tui_tree_widget::Tree::new(&tree_items)
        .expect("tree items have unique ids");
    let hidden_area = Rect::new(area.x, area.y, area.width, area.height);
    frame.render_stateful_widget(hidden_tree, hidden_area, &mut app.tree_state);

    let label_map = build_label_map(app);
    let flattened = app.tree_state.flatten(&tree_items);
    let selected_id = app.tree_state.selected();

    let mut items: Vec<ListItem> = Vec::new();
    let mut selected_index: Option<usize> = None;

    for (i, flat) in flattened.iter().enumerate() {
        let depth = flat.depth();
        let is_selected = flat.identifier == selected_id;
        if is_selected {
            selected_index = Some(i);
        }

        // Is this the last sibling at its depth?
        let is_last = {
            let mut last = true;
            for next in flattened.iter().skip(i + 1) {
                if next.depth() < depth {
                    break;
                }
                if next.depth() == depth {
                    last = false;
                    break;
                }
            }
            last
        };

        // Build tree connector prefix
        let mut prefix = String::new();
        for d in 0..depth {
            if d == depth - 1 {
                prefix.push_str(if is_last { "└─" } else { "├─" });
            } else {
                let has_more = has_more_siblings_at_depth(&flattened, i, d);
                prefix.push_str(if has_more { "│ " } else { "  " });
            }
        }

        let text = label_map
            .get(&flat.identifier)
            .cloned()
            .unwrap_or_else(|| format!("{:?}", flat.identifier));

        let style = if is_selected {
            if in_move_mode {
                Style::default().fg(FG).bg(RED).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(ORANGE).bg(BG1).add_modifier(Modifier::BOLD)
            }
        } else {
            Style::default().fg(FG2)
        };

        let cursor = if is_selected { "▸ " } else { "  " };
        let connector_style = if is_selected { style } else { Style::default().fg(BG2) };

        let line = Line::from(vec![
            Span::styled(cursor, style),
            Span::styled(prefix, connector_style),
            Span::styled(if depth > 0 { " " } else { "" }, style),
            Span::styled(text, style),
        ]);

        items.push(ListItem::new(line));
    }

    let list = List::new(items).block(block);
    let mut list_state = ListState::default();
    list_state.select(selected_index);
    frame.render_stateful_widget(list, area, &mut list_state);
}

fn has_more_siblings_at_depth<Identifier>(
    flattened: &[tui_tree_widget::Flattened<'_, Identifier>],
    current_idx: usize,
    target_depth: usize,
) -> bool {
    for item in flattened.iter().skip(current_idx + 1) {
        let d = item.depth();
        if d <= target_depth {
            return d == target_depth;
        }
    }
    false
}
