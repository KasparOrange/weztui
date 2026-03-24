# weztui

A terminal UI for managing WezTerm windows, tabs, panes, and sessions. Replaces WezTerm's flat InputSelector overlays with a proper tree-view interface featuring keyboard navigation, fuzzy search, live pane preview, and session save/restore.

Built with Rust + [ratatui](https://github.com/ratatui/ratatui) for instant startup.

## Features

- **Tree view** of all WezTerm windows, tabs, and panes with vim-style navigation
- **Fuzzy finder** (`/` or `weztui find`) — quick-switch to any pane by typing
- **Live pane preview** — see the terminal content of any pane before switching
- **Actions** — focus, rename, move, and close tabs/panes with keyboard shortcuts
- **Session management** — save and restore complete workspace layouts (`weztui save`, `weztui load`)
- **Workspace grouping** — windows grouped by WezTerm workspace when multiple exist
- **WezTerm plugin** — one-line install, auto-hides tab bar, status bar widget
- **Gruvbox theme** — warm orange color scheme

## Installation

```bash
cargo install --git https://github.com/KasparOrange/weztui
```

### WezTerm Plugin (recommended)

Add to your `~/.wezterm.lua`:

```lua
local weztui = wezterm.plugin.require 'https://github.com/KasparOrange/weztui'
weztui.apply_to_config(config)
```

This gives you:
- **Cmd+'** to launch weztui (configurable)
- Tab bar auto-hides while weztui is active
- Workspace and tab count in the status bar

### Manual Launch

```bash
weztui              # Tree view
weztui find         # Fuzzy finder
weztui find "vim"   # Pre-filled search
```

## Keybindings

| Key | Action |
|-----|--------|
| `j` / `k` / arrows | Navigate |
| `h` / `l` | Collapse / expand |
| `Enter` / `f` | Focus pane (and quit) |
| `r` | Rename tab or window |
| `m` | Move tab/pane to another window |
| `x` | Close (with confirmation) |
| `/` | Fuzzy search |
| `s` | Session picker |
| `S` | WezTerm settings editor (live preview) |
| `?` | Help |
| `q` / `Esc` | Quit |

## Session Management

```bash
weztui save my-project    # Save current layout
weztui load my-project    # Restore a saved layout
weztui sessions           # List saved sessions
weztui delete my-project  # Delete a session
```

Sessions are stored in `~/.config/weztui/sessions/` as JSON. They capture window positions, tab names, pane split layouts, and working directories.

## Settings Editor

Press `S` (Shift+s) to open the WezTerm settings editor. Changes apply in real-time via live preview.

- **7 categories**: Font, Colors, Window, Tab Bar, Cursor, Scrollback, Behavior
- **30 settings** covering the most useful WezTerm options
- **Live preview**: changes apply instantly as you adjust them
- **Save**: press `w` to persist settings across restarts
- **Reset**: press `r` to reset a modified setting to its saved value
- **Edit Lua**: press `e` to open `~/.wezterm.lua` in your editor

Settings are stored in `~/.config/weztui/settings.json` and loaded automatically by the companion Lua plugin on WezTerm startup.

## Plugin Configuration

All options are optional:

```lua
weztui.apply_to_config(config, {
    key = 'g',              -- Launch key (default: 'g')
    mods = 'CMD|SHIFT',     -- Modifiers (default: 'CMD|SHIFT')
    binary = nil,           -- Auto-detected, or explicit path
    status_bar = true,      -- Status bar widget (default: true)
    hide_tab_bar = true,    -- Hide tab bar while active (default: true)
})
```

## Contributing

Contributions welcome! Feel free to:

- Open an [issue](https://github.com/KasparOrange/weztui/issues) for bugs or feature requests
- Submit a [pull request](https://github.com/KasparOrange/weztui/pulls)
- Fork and make it your own

## Development

```bash
git clone https://github.com/KasparOrange/weztui
cd weztui
cargo run           # Debug build
cargo test          # Run tests (84 tests)
cargo build --release
```

## License

MIT
