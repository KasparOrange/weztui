# Fuzzy Finder — Quick-Switch to Any Tab ✅

## Goal

Press `/` (or launch `weztui find`) to get a fuzzy search across all tabs, panes, and workspaces. Type a few characters, hit Enter, and instantly jump to that tab — even across windows. Like telescope.nvim or fzf but for WezTerm tabs.

## Status

**Complete.** Implemented with `fuzzy-matcher` crate (SkimMatcherV2). Supports `/` from tree view and `weztui find [query]` CLI mode.

## Depends On

- MVP complete (data layer, WezTerm CLI wrapper)

## Design

### Search Sources

Each result is a pane, displayed with context:

```
Window: Main Dev  >  Tab: editor  >  Pane: nvim  (~/code/MitWare)
Window: Debug     >  Tab: logs    >  Pane: tail  (/var/log)
```

Fuzzy match against: tab title, window title, pane process name, pane working directory, workspace name.

### Ranking

1. Exact prefix match on tab title (highest)
2. Fuzzy match on tab title
3. Fuzzy match on working directory basename
4. Fuzzy match on full cwd path
5. Fuzzy match on process name

Boost recently-active tabs (MRU ordering as tiebreaker).

### UX Flow

1. Press `/` in tree view (or launch `weztui find` directly)
2. Input field at the top, results below — filtered live as you type
3. Arrow keys or Ctrl+N/P to navigate results
4. Enter activates the selected pane and exits weztui
5. Esc returns to tree view (or exits if launched in find mode)

### Direct Launch Mode

```bash
weztui find              # Open straight to fuzzy finder
weztui find "mitware"    # Pre-fill search query
```

Useful as a WezTerm keybinding for quick tab switching without the full manager.

## Implementation Notes

- Use a simple fuzzy matching algorithm (sublime-style: characters must appear in order, not necessarily contiguous). Consider the `fuzzy-matcher` or `nucleo` crate.
- Keep the index in memory — the full pane list is small enough that re-filtering on every keystroke is fine
- Debounce is unnecessary given the data size (<100 items typically)

## Stretch Goals

- Preview pane: show the last few lines of scrollback for the selected pane (via `wezterm cli get-text --pane-id P`)
- Search within pane scrollback content (heavier, opt-in)
- Frecency-based ranking (track usage over time, persist to disk)
