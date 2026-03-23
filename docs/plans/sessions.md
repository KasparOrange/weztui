# Session Management ✅

## Goal

Save and restore complete WezTerm workspace layouts — which windows exist, their tabs, pane splits, working directories, and running commands. Like tmux-resurrect but for WezTerm.

## Status

**Complete.** CLI commands: `weztui save <name>`, `weztui load <name>`, `weztui sessions`, `weztui delete <name>`. Split topology reconstructed from pane geometry into a binary tree, restored via `wezterm cli spawn` and `split-pane`.

## Depends On

- MVP complete

## Features

### Save Session

- Capture full state: windows, tabs, pane split topology, cwd per pane, tab/window names
- Save to `~/.config/weztui/sessions/<name>.json`
- Auto-save on quit (optional)
- Named snapshots: `weztui save "project-x"`

### Restore Session

- Recreate windows, tabs, splits from a saved session
- `cd` to each pane's saved working directory
- Optionally re-run the last command (configurable per-pane)
- Handle conflicts: merge into existing windows or replace

### Session Picker

- TUI screen listing all saved sessions with metadata (date, window count, tab names)
- Preview: show the layout as an ASCII diagram before restoring
- Delete old sessions

## Data Model

```json
{
  "name": "project-x",
  "saved_at": "2026-03-22T18:00:00Z",
  "windows": [
    {
      "title": "Main Dev",
      "tabs": [
        {
          "title": "editor",
          "panes": [
            {
              "cwd": "/Users/konrad/code/MitWare",
              "command": "nvim",
              "split": "root"
            },
            {
              "cwd": "/Users/konrad/code/MitWare",
              "command": null,
              "split": { "direction": "right", "size_percent": 30 }
            }
          ]
        }
      ]
    }
  ]
}
```

## Open Questions

- How to capture split topology? `wezterm cli list` gives pane positions (left/top/width/height) but not the split tree. Need to reconstruct from geometry.
- Should we save/restore environment variables?
- How to handle "the same session restored twice" — deduplicate or allow duplicates?
- Integration with workspaces (`wezterm cli rename-workspace`)?

## Prior Art

- [resurrect.wezterm](https://github.com/MLFlexer/resurrect.wezterm) — Lua plugin that does this inside WezTerm. Study its approach to split topology reconstruction.
- tmux-resurrect / tmux-continuum — the gold standard for terminal session persistence
