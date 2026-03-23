# Visual Pane Layouts ✅

## Goal

Show an ASCII/box-drawing preview of how panes are arranged within a tab. Let users see the spatial layout at a glance and potentially rearrange splits visually.

## Status

**Complete.** Preview panel shows when a tab or pane is selected. Proportional scaling from WezTerm geometry, box-drawing borders, active/selected pane highlighting.

## Depends On

- MVP complete

## Design

### Layout Preview

When a tab is selected in the tree, show its pane layout in a side panel:

```
Tab: "editor" (3 panes)
+---------------------+---------+
|                     |  tests  |
|       nvim          |  (zsh)  |
|    ~/code/MitWare   +---------+
|                     |  logs   |
|                     |  (tail) |
+---------------------+---------+
```

### How to Reconstruct the Layout

`wezterm cli list --format json` gives each pane's position:
```json
{
  "pane_id": 3,
  "tab_id": 1,
  "left": 0, "top": 0,
  "width": 80, "height": 24
}
```

From the positions, reconstruct the split tree:
1. Sort panes by (top, left)
2. Find the split axis: if two panes share a top edge but differ in left, it is a vertical split. If they share a left edge but differ in top, it is a horizontal split.
3. Recursively partition until each leaf contains one pane.

### Rendering

- Use box-drawing characters for borders
- Show pane process name and truncated cwd inside each cell
- Highlight the active pane
- Scale proportionally to the available panel width

### Future: Interactive Rearrangement

- Arrow keys to select a pane within the layout view
- `s` to swap two panes
- `r` to resize (shift the split boundary)
- These map to `wezterm cli adjust-pane-size` and pane swap commands

## Open Questions

- How much detail to show in small panes? Truncation strategy.
- Should the layout preview be always visible or toggle-able?
- How to handle deeply nested splits (3+ levels)?

## Prior Art

- tmux `display-panes` shows pane numbers overlaid on the terminal
- i3/sway show tiling layouts — similar visual language
