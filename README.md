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
| `,` | WezTerm settings dialog (live, saved at once) |
| `?` | Help |
| `q` / `Esc` | Quit |

## Session Management

```bash
weztui save my-project    # Save current layout
weztui load my-project    # Restore a saved layout
weztui sessions           # List saved sessions
weztui delete my-project  # Delete a session
```

Sessions are stored in `~/.local/state/weztui/sessions/` as JSON (state the app writes; safe to delete). They capture window positions, tab names, pane split layouts, and working directories.

## Settings Editor

Press `,` to open the WezTerm settings dialog (`S` still works). Every change applies to WezTerm at once and is saved immediately.

- **7 categories**: Font, Colors, Window, Tab Bar, Cursor, Scrollback, Behavior
- **30 settings** covering the most useful WezTerm options, each with a one-line description
- **One grammar**: `j`/`k` move, `Enter`/`l`/`→` go deeper (categories → rows → value), `Esc`/`h`/`←` go out, `q` or `,` close
- **On a row**: a toggle flips in place; a choice unfolds into its options; a number becomes a slider (`k` more, `j` less). While a value is open it is previewed live — `Enter`/`h` keeps and saves it, `Esc` puts back the value from before
- **Edit Lua**: press `e` on a row to open `~/.wezterm.lua` in your editor

Settings live in `~/.config/weztui/config.toml`, in the `[wezterm]` table (key names are WezTerm's own; a key that is not set is left to WezTerm):

```toml
#:schema ./config.schema.json
[wezterm]
font_size = 14.0
window_decorations = "TITLE | RESIZE"
```

The dialog writes one key at a time, so comments, order and keys it does not know survive; editing the file by hand is fine. `config.schema.json` next to it gives editor validation and completion (Taplo). The companion Lua plugin reads the same file when WezTerm starts or reloads its config.

## Plugin Configuration

All options are optional:

```lua
weztui.apply_to_config(config, {
    key = 'g',              -- Launch key (default: 'g')
    mods = 'CMD|SHIFT',     -- Modifiers (default: 'CMD|SHIFT')
    binary = nil,           -- Auto-detected, or explicit path
    status_bar = true,      -- Status bar widget (default: true)
    hide_tab_bar = true,    -- Hide tab bar while active (default: true)
    log = nil,              -- false = never log; '/path/to/mwlog' = log through that CLI
})
```

## Doctor and logs

```bash
weztui doctor           # WezTerm reachable? settings file, sessions, Claude sessions, log shipper counters
weztui doctor --json    # the same for tools (exit 1 when WezTerm or the settings file fail)
```

weztui logs what it does — commands, keys that do something, every `wezterm cli` call with its
duration and outcome, session save/restore, settings changes — to a daily file in
`~/.local/state/weztui/logs/` (7 kept) and never to the terminal. On the author's machines the
same lines go to a private log server (MwLog) when `~/.config/mwlog/weztui.env` exists; without
that file nothing leaves the machine. The plugin logs its few events (toggle, crash recovery)
only where the `mwlog` CLI is installed. Areas, messages and the trace id: [docs/logging.md](docs/logging.md).

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
cargo test          # Run tests
cargo build --release
```

The build expects two sibling checkouts next to this one: `../stack-settings` (settings file) and `../mwlog-rs` (logging).

## License

MIT
