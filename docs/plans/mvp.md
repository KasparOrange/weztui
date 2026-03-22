# MVP — Core Tree View & Tab Management

## Goal

Replace the Lua-based InputSelector manager (`~/.config/wezterm/manager.lua`) with a proper TUI that launches in a WezTerm pane and provides a tree view of all windows, tabs, and panes with keyboard-driven actions.

## Success Criteria

- Launches in <20ms, shows tree immediately
- Tree view: Windows > Tabs > Panes, collapsible
- Move any tab to any window (or new window)
- Rename tabs and windows inline
- Close tabs and windows
- Focus/activate any tab from the tree
- Quit returns to the previous pane seamlessly

## Implementation Phases

### Phase 1: Data Layer ✅

- [x] Define model structs: `WezWindow`, `WezTab`, `WezPane`
- [x] Implement `wezterm cli list --format json` parser
- [x] Build tree structure from flat pane list (group by window_id, then tab_id)
- [x] Detect "current" window/tab/pane from `WEZTERM_PANE` env var

### Phase 2: Basic TUI ✅

- [x] Terminal setup/teardown with crossterm (raw mode, alternate screen)
- [x] App struct with state (selected node, tree expansion state)
- [x] Event loop: poll for key events, re-render on change
- [x] Render the tree using `tui-tree-widget`
- [x] Style with Gruvbox colors to match the user's WezTerm theme
- [x] Status bar showing current selection info

### Phase 3: Actions ✅

- [x] Enter on a tab/window opens an action menu (right panel or popup)
- [x] Move tab: `m` key, then pick destination window from a list
- [x] Rename: `r` key, inline text input
- [x] Close: `x` key with confirmation
- [x] Focus: Enter (or `f`) activates the pane and quits weztui

### Phase 4: Polish ✅

- [x] Refresh tree data on focus (in case external changes happened)
- [x] Show pane working directory and process in tree labels
- [x] Highlight active tabs vs inactive
- [x] Handle edge cases: single window, no tabs, WezTerm not running
- [x] Error messages in status bar instead of panicking

## WezTerm Keybinding Integration

After MVP, update `~/.config/wezterm/keys.lua` Cmd+Shift+G to spawn `weztui` in a floating overlay (once WezTerm supports floating panes) or in a new split:

```lua
{
    key = 'g',
    mods = 'CMD|SHIFT',
    action = act.SpawnCommandInNewTab {
        args = { '/path/to/weztui' },
    },
}
```

## Non-Goals for MVP

- Session save/restore
- Fuzzy search
- Visual pane layout preview
- Custom themes beyond matching Gruvbox
- Plugin system
