# WezTerm Integration

## Goal

Make weztui a drop-in enhancement for WezTerm. One command to install the keybinding, seamless launch and exit, and optional auto-features like session autosave.

## Depends On

- MVP complete

## Installation Modes

### Manual (MVP)

User adds a keybinding to their WezTerm config:

```lua
{
    key = 'g',
    mods = 'CMD|SHIFT',
    action = wezterm.action_callback(function(win, pane)
        local tab, _, _ = pane:move_to_new_tab()
        win:perform_action(wezterm.action.SpawnCommandInNewTab {
            args = { '/path/to/weztui' },
        }, pane)
    end),
}
```

### Auto-Install

```bash
weztui install    # Detects WezTerm config, adds keybinding
weztui uninstall  # Removes the keybinding
```

This would:
1. Find `~/.wezterm.lua` or `~/.config/wezterm/wezterm.lua`
2. Detect if using a keys module (like `keys.lua`)
3. Add a keybinding entry that spawns `weztui`
4. Write the weztui binary path based on `which weztui` or the release location

### Launch Strategy

Options for how weztui appears:

| Strategy | Pros | Cons |
|---|---|---|
| **New tab** | Simple, full-screen space | Tab flash on open/close |
| **Split pane** | See context alongside | Takes space from current work |
| **Overlay** (future) | Best UX, no tab flash | Requires WezTerm floating panes (PR #5576) |

MVP: launch in a new tab. When weztui exits, the tab auto-closes and the user is back where they were.

## Seamless Exit

When the user selects "focus tab X" in weztui:
1. weztui calls `wezterm cli activate-pane --pane-id P`
2. weztui exits (the spawned tab auto-closes since its process ended)
3. The target tab is now active

The user sees: press keybinding -> tree appears -> pick tab -> instantly there.

## Title Persistence Bridge

The Lua-based `manager.lua` stores custom tab titles in `wezterm.GLOBAL.pane_titles`. weztui cannot read Lua globals directly, but it could:

1. Read titles via a sidecar file (`~/.config/weztui/titles.json`) that the Lua config writes to
2. Or: weztui sets titles via `wezterm cli set-tab-title` which the Lua `format-tab-title` handler picks up

Approach 2 is simpler and already works with the existing Lua setup.

## Configuration

```toml
# ~/.config/weztui/config.toml

[keybinding]
launch_key = "g"
launch_mods = "CMD|SHIFT"

[appearance]
theme = "gruvbox-dark"    # Match WezTerm theme
show_pane_preview = true

[sessions]
autosave = true
autosave_interval_minutes = 5
session_dir = "~/.config/weztui/sessions"

[behavior]
quit_after_action = true   # Exit after focus/move/rename
confirm_close = true       # Ask before closing tabs/windows
```

## Open Questions

- Should weztui have its own config file, or piggyback on WezTerm's Lua config via user vars?
- How to detect the weztui binary path reliably after `cargo install`?
- Should `weztui install` back up the existing config first?
