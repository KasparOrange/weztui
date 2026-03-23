mod layout_preview;
mod popup;
mod search;
mod status;
mod tree;

use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::Block;
use ratatui::Frame;

use crate::app::{App, Mode, NodeId};
use crate::model::WezPane;

// Gruvbox dark palette
pub const BG: ratatui::style::Color = ratatui::style::Color::Rgb(0x28, 0x28, 0x28);
pub const BG1: ratatui::style::Color = ratatui::style::Color::Rgb(0x3c, 0x38, 0x36);
pub const BG2: ratatui::style::Color = ratatui::style::Color::Rgb(0x50, 0x49, 0x45);
pub const FG: ratatui::style::Color = ratatui::style::Color::Rgb(0xeb, 0xdb, 0xb2);
pub const FG2: ratatui::style::Color = ratatui::style::Color::Rgb(0xd5, 0xc4, 0xa1);
pub const ORANGE: ratatui::style::Color = ratatui::style::Color::Rgb(0xfe, 0x80, 0x19);
pub const YELLOW: ratatui::style::Color = ratatui::style::Color::Rgb(0xfa, 0xbd, 0x2f);
pub const GREEN: ratatui::style::Color = ratatui::style::Color::Rgb(0xb8, 0xbb, 0x26);
pub const RED: ratatui::style::Color = ratatui::style::Color::Rgb(0xfb, 0x49, 0x34);
pub const AQUA: ratatui::style::Color = ratatui::style::Color::Rgb(0x8e, 0xc0, 0x7c);
#[allow(dead_code)]
pub const BLUE: ratatui::style::Color = ratatui::style::Color::Rgb(0x83, 0xa5, 0x98);
#[allow(dead_code)]
pub const PURPLE: ratatui::style::Color = ratatui::style::Color::Rgb(0xd3, 0x86, 0x9b);

/// Resolve which tab to preview based on the current tree selection.
fn resolve_preview_info(app: &App) -> Option<(Vec<WezPane>, String, Option<u64>)> {
    let selected_pane_id = match app.tree_state.selected().last() {
        Some(NodeId::Pane(id)) => Some(*id),
        _ => None,
    };

    let tab = match app.tree_state.selected().last()? {
        NodeId::Tab(tab_id) => app.find_tab(*tab_id),
        NodeId::Pane(pane_id) => app.find_pane_tab(*pane_id),
        NodeId::Window(_) => None,
    }?;

    Some((
        tab.panes.clone(),
        tab.title.clone().unwrap_or_default(),
        selected_pane_id,
    ))
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    frame.render_widget(Block::default().style(Style::default().bg(BG)), area);

    let [title_area, main_area, status_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(area);

    // Title changes based on mode
    let title_text = match &app.mode {
        Mode::Search { .. } => " Find",
        _ => " weztui",
    };
    let title = Line::from(title_text).style(
        Style::default()
            .fg(ORANGE)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_widget(title, title_area);

    // Extract preview info with shared borrows before mutable rendering
    let preview_info = if matches!(app.mode, Mode::Normal) {
        resolve_preview_info(app)
    } else {
        None
    };

    // Main area: search, tree, or tree + preview
    match &app.mode {
        Mode::Search { query, cursor, entries, results, selected_index, .. } => {
            search::render_search(
                frame, main_area, query, *cursor, entries, results, *selected_index,
            );
        }
        _ => {
            if let Some((ref panes, ref tab_title, sel_pane)) = preview_info {
                if main_area.width >= 40 {
                    let [tree_area, preview_area] = Layout::horizontal([
                        Constraint::Percentage(60),
                        Constraint::Percentage(40),
                    ])
                    .areas(main_area);
                    tree::render_tree(frame, tree_area, app);
                    layout_preview::render_layout_preview(
                        frame,
                        preview_area,
                        panes,
                        tab_title,
                        sel_pane,
                    );
                } else {
                    tree::render_tree(frame, main_area, app);
                }
            } else {
                tree::render_tree(frame, main_area, app);
            }
        }
    }

    status::render_status(frame, status_area, app);

    // Overlay popups for modal modes
    match &app.mode {
        Mode::Move { window_choices, selected_index } => {
            popup::render_move_popup(frame, main_area, window_choices, *selected_index);
        }
        Mode::Confirm { label, .. } => {
            popup::render_confirm_popup(frame, main_area, label);
        }
        _ => {}
    }
}
