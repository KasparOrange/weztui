# weztui - WezTerm TUI Manager

## What This Is

A terminal UI for managing WezTerm windows, tabs, panes, and sessions. Launched via keybinding, it replaces WezTerm's flat InputSelector overlays with a proper tree-view interface featuring keyboard navigation, fuzzy search, and visual pane layouts.

Built with Rust + ratatui for instant startup (~5-15ms).

## Plans

Feature plans and design docs live in `docs/plans/`:

- [MVP](docs/plans/mvp.md) — Core tree view, tab/window management, the minimum to replace the Lua manager
- [Session Management](docs/plans/sessions.md) — Save/restore window layouts and working directories
- [Fuzzy Finder](docs/plans/fuzzy-finder.md) — Quick-switch to any tab by typing
- [Visual Layouts](docs/plans/visual-layouts.md) — ASCII preview of pane split layouts, drag-to-rearrange
- [WezTerm Integration](docs/plans/wezterm-integration.md) — Auto-install keybinding into WezTerm Lua config

## Technology

| Crate | Purpose |
|---|---|
| ratatui | TUI framework (rendering, layout, widgets) |
| crossterm | Terminal backend (input, raw mode, colors) |
| tui-tree-widget | Collapsible tree view widget |
| serde / serde_json | Parse `wezterm cli list --format json` |
| clap | CLI argument parsing |
| color-eyre | Error handling with pretty backtraces |

## Architecture

```
src/
  main.rs          — Entry point, CLI args, terminal setup/teardown
  app.rs           — App state, event loop, key handling
  wezterm.rs       — Wrapper around `wezterm cli` commands
  ui/
    mod.rs         — Top-level render function
    tree.rs        — Window/tab/pane tree view
    actions.rs     — Action menu panel
    status.rs      — Status bar
  model.rs         — Data model (Window, Tab, Pane structs)
  session.rs       — Session save/restore (future)
```

## How It Talks to WezTerm

All interaction goes through the `wezterm cli` subcommands:

| Command | Purpose |
|---|---|
| `wezterm cli list --format json` | Enumerate all windows/tabs/panes |
| `wezterm cli move-pane-to-new-tab --window-id W --pane-id P` | Move pane to another window |
| `wezterm cli activate-pane --pane-id P` | Focus a specific pane/tab |
| `wezterm cli set-tab-title TITLE` | Rename a tab |
| `wezterm cli set-window-title TITLE` | Rename a window |
| `wezterm cli split-pane` | Create splits |
| `wezterm cli spawn` | Create new tabs/windows |
| `wezterm cli kill-pane --pane-id P` | Close a pane |

JSON from `wezterm cli list` provides: `window_id`, `tab_id`, `pane_id`, `workspace`, `title`, `cwd`, `size` (rows/cols).

## Code Style

- Use `color_eyre::Result` for all fallible functions
- Structs with public fields for data models, methods for behavior
- Keep `wezterm.rs` as the only module that shells out to `wezterm` — everything else works with the model structs
- Group imports: std, external crates, local modules
- `snake_case` for everything, idiomatic Rust

## Testing

Write tests wherever possible and reasonable. Focus on:

- **Pure logic**: data transformations, tree building, state lookups — always test these
- **State mutations**: key handling, selection changes — test via constructing state and asserting outcomes
- **Skip**: UI rendering and `wezterm cli` wrappers — these depend on terminal/subprocess I/O and aren't worth mocking

Run tests with `cargo test`. Place unit tests in `#[cfg(test)] mod tests` at the bottom of each module.

## Documentation

Always keep `README.md` and `CLAUDE.md` up to date when adding features, changing keybindings, or modifying CLI commands. The README is the user-facing reference; CLAUDE.md is the developer reference. If you add a feature, document it before committing.

## Warnings

`#![deny(warnings)]` is set in `main.rs` — all warnings are compile errors. Do not leave unused imports, dead code, or other warnings. Use `#[allow(dead_code)]` only for fields/constants reserved for planned future features.

## Build & Run

```bash
cargo run                    # Debug build
cargo build --release        # Optimized binary (~3MB)
./target/release/weztui      # Launch directly
```

## Key Bindings (in the TUI)

Design target (implement iteratively):

| Key | Action |
|---|---|
| j/k or arrows | Navigate tree |
| Enter | Expand/collapse node, or confirm action |
| / | Fuzzy search |
| m | Move selected tab to another window |
| r | Rename selected tab/window |
| x | Close selected tab/window |
| q / Esc | Quit |
| ? | Show help |
| Tab | Switch between tree panel and action panel |

## Integration with WezTerm Lua Config

The existing Lua-based manager lives in `~/.config/wezterm/manager.lua` (in the MitWare sibling project's WezTerm config). Once weztui reaches MVP, the Lua manager's Cmd+Shift+G binding should launch `weztui` instead of the InputSelector overlay.
