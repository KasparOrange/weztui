# Companion Lua Plugin — weztui.wezterm

## Goal

A WezTerm Lua plugin that makes weztui a first-class citizen in WezTerm. Installable with one line, handles launching, IPC, status bar integration, and session restore.

## Installation (end user)

```lua
local weztui = wezterm.plugin.require 'https://github.com/KasparOrange/weztui.wezterm'
weztui.apply_to_config(config)
```

## Features

### 1. Keybinding & Launch

- Registers Cmd+Shift+G (configurable) to launch weztui in a new tab
- Detects weztui binary path via `wezterm.home_dir` + `/.cargo/bin/weztui` or user override
- Hides tab bar while weztui is active via `set_config_overrides`
- Restores tab bar when weztui exits

### 2. User Var IPC (weztui → WezTerm)

weztui sends commands to WezTerm via OSC 1337 user vars:
```
\033]1337;SetUserVar=weztui_cmd=BASE64_JSON\007
```

The plugin listens to `user-var-changed` and executes actions:
- `activate_pane` — focus a pane (faster than wezterm cli)
- `set_tab_title` / `set_window_title` — rename
- `hide_tab_bar` / `show_tab_bar` — toggle tab bar visibility

This replaces some `wezterm cli` calls with zero-latency IPC.

### 3. Status Bar Widget

Uses `update-status` event to render workspace info in the right status:
- Current workspace name
- Window/tab/pane counts
- Active process name
- Uses Nerd Font icons if available

### 4. Auto-Session Restore

On `gui-startup`, check for a saved session and offer to restore:
- Reads `~/.config/weztui/sessions/` for saved sessions
- If `last-session.json` exists, auto-restore it
- Configurable: auto-restore, prompt, or disabled

## Plugin Structure

```
weztui.wezterm/
  plugin/
    init.lua          — Entry point, exports apply_to_config
  README.md
```

Single file plugin — all logic in `init.lua` (~200 lines).

## Configuration (user-facing)

```lua
weztui.apply_to_config(config, {
    key = 'g',                    -- launch key (default: g)
    mods = 'CMD|SHIFT',          -- launch modifiers (default: CMD|SHIFT)
    binary = nil,                 -- auto-detect, or explicit path
    status_bar = true,            -- show status widget (default: true)
    auto_restore = false,         -- restore last session on startup (default: false)
    hide_tab_bar = true,          -- hide tab bar while weztui active (default: true)
})
```

## Rust-Side Changes (weztui binary)

### User Var Output

Add a function to emit OSC 1337 user vars:
```rust
fn emit_user_var(key: &str, value: &str) {
    print!("\x1b]1337;SetUserVar={}={}\x07", key, base64(value));
}
```

When weztui starts: `emit_user_var("weztui_active", "true")`
When weztui exits: `emit_user_var("weztui_active", "false")`

The plugin uses this to toggle tab bar visibility.

### Optional: Fast IPC for Actions

Instead of `wezterm cli activate-pane --pane-id P`:
```rust
emit_user_var("weztui_cmd", r#"{"action":"activate_pane","pane_id":42}"#);
```

The Lua plugin catches this and calls `window:perform_action()` directly — no subprocess overhead.

## Repository

Separate repo: `KasparOrange/weztui.wezterm` (WezTerm plugin convention is `name.wezterm`).

## Open Questions

- Should the status bar widget be opt-in or opt-out?
- Should auto-restore use the weztui session format or resurrect.wezterm format for compatibility?
- Should the plugin offer its own InputSelector-based quick-switch (for users who don't want the full TUI)?
