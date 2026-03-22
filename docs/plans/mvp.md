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

### Phase 1: Data Layer

- [ ] Define model structs: `WezWindow`, `WezTab`, `WezPane`
- [ ] Implement `wezterm cli list --format json` parser
- [ ] Build tree structure from flat pane list (group by window_id, then tab_id)
- [ ] Detect "current" window/tab/pane from `WEZTERM_PANE` env var

### Phase 2: Basic TUI

- [ ] Terminal setup/teardown with crossterm (raw mode, alternate screen)
- [ ] App struct with state (selected node, tree expansion state)
- [ ] Event loop: poll for key events, re-render on change
- [ ] Render the tree using `tui-tree-widget`
- [ ] Style with Gruvbox colors to match the user's WezTerm theme
- [ ] Status bar showing current selection info

### Phase 3: Actions

- [ ] Enter on a tab/window opens an action menu (right panel or popup)
- [ ] Move tab: `m` key, then pick destination window from a list
- [ ] Rename: `r` key, inline text input
- [ ] Close: `x` key with confirmation
- [ ] Focus: Enter (or `f`) activates the pane and quits weztui

### Phase 4: Polish

- [ ] Refresh tree data on focus (in case external changes happened)
- [ ] Show pane working directory and process in tree labels
- [ ] Highlight active tabs vs inactive
- [ ] Handle edge cases: single window, no tabs, WezTerm not running
- [ ] Error messages in status bar instead of panicking

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
