mod pane_preview;
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
use crate::wezterm;

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

/// Resolve which pane to preview based on the current tree selection.
/// Returns (pane_title, pane_text_content) if a pane or tab is selected.
fn resolve_preview(app: &App) -> Option<(String, String)> {
    let pane_id = match app.tree_state.selected().last()? {
        NodeId::Pane(id) => *id,
        NodeId::Tab(tab_id) => {
            // Preview the first pane in the tab
            let tab = app.find_tab(*tab_id)?;
            tab.panes.first()?.pane_id
        }
        NodeId::Window(_) => return None,
    };

    // Don't preview the pane running weztui itself
    if Some(pane_id) == app.current_pane_id {
        return None;
    }

    let title = app.windows.iter()
        .flat_map(|w| &w.tabs)
        .flat_map(|t| &t.panes)
        .find(|p| p.pane_id == pane_id)
        .map(|p| p.title.clone())
        .unwrap_or_default();

    let content = wezterm::get_pane_text(pane_id).unwrap_or_default();

    Some((title, content))
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

    // Extract preview with shared borrows before mutable rendering
    let preview = if matches!(app.mode, Mode::Normal) {
        resolve_preview(app)
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
            if let Some((ref title, ref content)) = preview {
                if main_area.width >= 40 {
                    let [tree_area, preview_area] = Layout::horizontal([
                        Constraint::Percentage(50),
                        Constraint::Percentage(50),
                    ])
                    .areas(main_area);
                    tree::render_tree(frame, tree_area, app);
                    pane_preview::render_pane_preview(frame, preview_area, title, content);
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
