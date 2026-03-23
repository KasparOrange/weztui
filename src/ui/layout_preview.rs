use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use super::{BG, BG2, FG, FG2, ORANGE, AQUA};
use crate::model::WezPane;

#[derive(Debug, Clone)]
pub struct MappedPane {
    pub title: String,
    pub cwd_short: String,
    pub is_active: bool,
    pub is_selected: bool,
    pub rect: Rect,
}

pub fn scale_panes_to_rect(
    panes: &[WezPane],
    target: Rect,
    selected_pane_id: Option<u64>,
) -> Vec<MappedPane> {
    if panes.is_empty() || target.width == 0 || target.height == 0 {
        return Vec::new();
    }

    let total_width = panes.iter().map(|p| p.left + p.width).max().unwrap_or(0);
    let total_height = panes.iter().map(|p| p.top + p.height).max().unwrap_or(0);

    if total_width == 0 || total_height == 0 {
        return Vec::new();
    }

    panes
        .iter()
        .map(|pane| {
            let x = target.x
                + ((pane.left as u32 * target.width as u32) / total_width as u32) as u16;
            let y = target.y
                + ((pane.top as u32 * target.height as u32) / total_height as u32) as u16;

            let right = target.x
                + (((pane.left + pane.width) as u32 * target.width as u32) / total_width as u32)
                    as u16;
            let bottom = target.y
                + (((pane.top + pane.height) as u32 * target.height as u32) / total_height as u32)
                    as u16;

            let available_w = (target.x + target.width).saturating_sub(x);
            let available_h = (target.y + target.height).saturating_sub(y);
            let w = right.saturating_sub(x).max(3).min(available_w);
            let h = bottom.saturating_sub(y).max(3).min(available_h);

            let cwd_short = pane
                .cwd
                .as_deref()
                .and_then(|c| c.rsplit('/').next())
                .unwrap_or("~")
                .to_string();

            MappedPane {
                title: pane.title.clone(),
                cwd_short,
                is_active: pane.is_active,
                is_selected: selected_pane_id == Some(pane.pane_id),
                rect: Rect::new(x, y, w, h),
            }
        })
        .collect()
}

pub fn render_layout_preview(
    frame: &mut Frame,
    area: Rect,
    panes: &[WezPane],
    tab_title: &str,
    selected_pane_id: Option<u64>,
) {
    let title = if tab_title.is_empty() {
        " Layout ".to_string()
    } else {
        format!(" {} ", tab_title)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BG2))
        .title(title)
        .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(BG).fg(FG));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 8 || inner.height < 4 {
        let msg = Paragraph::new("(narrow)").style(Style::default().fg(FG2));
        frame.render_widget(msg, inner);
        return;
    }

    let mapped = scale_panes_to_rect(panes, inner, selected_pane_id);

    for mp in &mapped {
        let border_color = if mp.is_selected {
            ORANGE
        } else if mp.is_active {
            AQUA
        } else {
            BG2
        };

        let pane_title = truncate(&mp.title, mp.rect.width.saturating_sub(2) as usize);
        let pane_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(pane_title)
            .title_style(Style::default().fg(FG).add_modifier(Modifier::BOLD));

        let pane_inner = pane_block.inner(mp.rect);
        frame.render_widget(pane_block, mp.rect);

        if pane_inner.width > 0 && pane_inner.height > 0 {
            let cwd_text = truncate(&mp.cwd_short, pane_inner.width as usize);
            let cwd_para = Paragraph::new(cwd_text).style(Style::default().fg(FG2));
            frame.render_widget(cwd_para, pane_inner);
        }
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len > 1 {
        format!("{}\u{2026}", &s[..max_len - 1])
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane_with_geometry(
        id: u64,
        title: &str,
        left: u64,
        top: u64,
        w: u64,
        h: u64,
    ) -> WezPane {
        WezPane {
            pane_id: id,
            title: title.to_string(),
            cwd: Some(format!("/home/user/{title}")),
            is_active: false,
            left,
            top,
            width: w,
            height: h,
        }
    }

    #[test]
    fn single_pane_fills_target() {
        let panes = vec![pane_with_geometry(1, "nvim", 0, 0, 80, 24)];
        let target = Rect::new(0, 0, 40, 12);
        let result = scale_panes_to_rect(&panes, target, None);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].rect, Rect::new(0, 0, 40, 12));
    }

    #[test]
    fn vertical_split_two_panes() {
        let panes = vec![
            pane_with_geometry(1, "nvim", 0, 0, 40, 24),
            pane_with_geometry(2, "zsh", 41, 0, 39, 24),
        ];
        let target = Rect::new(0, 0, 40, 12);
        let result = scale_panes_to_rect(&panes, target, None);
        assert_eq!(result.len(), 2);
        // Left pane should be roughly left half
        assert!(result[0].rect.x == 0);
        assert!(result[0].rect.width > 0);
        // Right pane should start after left
        assert!(result[1].rect.x > 0);
        assert!(result[1].rect.x + result[1].rect.width <= 40);
    }

    #[test]
    fn horizontal_split_two_panes() {
        let panes = vec![
            pane_with_geometry(1, "top", 0, 0, 80, 12),
            pane_with_geometry(2, "bottom", 0, 13, 80, 11),
        ];
        let target = Rect::new(0, 0, 40, 12);
        let result = scale_panes_to_rect(&panes, target, None);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].rect.y, 0);
        assert!(result[1].rect.y > 0);
    }

    #[test]
    fn three_way_l_shape() {
        // Large pane on left, two stacked on right
        let panes = vec![
            pane_with_geometry(1, "nvim", 0, 0, 60, 24),
            pane_with_geometry(2, "tests", 61, 0, 19, 12),
            pane_with_geometry(3, "logs", 61, 13, 19, 11),
        ];
        let target = Rect::new(0, 0, 40, 12);
        let result = scale_panes_to_rect(&panes, target, None);
        assert_eq!(result.len(), 3);
        // All rects should be within target
        for mp in &result {
            assert!(mp.rect.x + mp.rect.width <= target.x + target.width);
            assert!(mp.rect.y + mp.rect.height <= target.y + target.height);
        }
    }

    #[test]
    fn empty_panes_returns_empty() {
        let result = scale_panes_to_rect(&[], Rect::new(0, 0, 40, 12), None);
        assert!(result.is_empty());
    }

    #[test]
    fn zero_size_target_returns_empty() {
        let panes = vec![pane_with_geometry(1, "nvim", 0, 0, 80, 24)];
        let result = scale_panes_to_rect(&panes, Rect::new(0, 0, 0, 0), None);
        assert!(result.is_empty());
    }

    #[test]
    fn minimum_size_enforced() {
        // Two equal panes in a target wide enough for both to get >= 3
        let panes = vec![
            pane_with_geometry(1, "left", 0, 0, 40, 24),
            pane_with_geometry(2, "right", 40, 0, 40, 24),
        ];
        let target = Rect::new(0, 0, 40, 12);
        let result = scale_panes_to_rect(&panes, target, None);
        for mp in &result {
            assert!(mp.rect.width >= 3, "pane width should be at least 3");
            assert!(mp.rect.height >= 3, "pane height should be at least 3");
        }
    }

    #[test]
    fn no_gaps_between_adjacent_panes() {
        // Two panes sharing an edge (pane A right edge = pane B left edge in scaled coords)
        let panes = vec![
            pane_with_geometry(1, "left", 0, 0, 40, 24),
            pane_with_geometry(2, "right", 40, 0, 40, 24),
        ];
        let target = Rect::new(0, 0, 40, 12);
        let result = scale_panes_to_rect(&panes, target, None);
        assert_eq!(result.len(), 2);
        let right_edge_a = result[0].rect.x + result[0].rect.width;
        let left_edge_b = result[1].rect.x;
        assert_eq!(right_edge_a, left_edge_b, "adjacent panes should share edge");
    }

    #[test]
    fn selected_pane_id_propagated() {
        let panes = vec![
            pane_with_geometry(1, "a", 0, 0, 40, 24),
            pane_with_geometry(2, "b", 40, 0, 40, 24),
        ];
        let result = scale_panes_to_rect(&panes, Rect::new(0, 0, 40, 12), Some(2));
        assert!(!result[0].is_selected);
        assert!(result[1].is_selected);
    }

    #[test]
    fn is_active_propagated() {
        let mut pane = pane_with_geometry(1, "nvim", 0, 0, 80, 24);
        pane.is_active = true;
        let result = scale_panes_to_rect(&[pane], Rect::new(0, 0, 40, 12), None);
        assert!(result[0].is_active);
    }
}
